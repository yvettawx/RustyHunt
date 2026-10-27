//! Finding and opening processes, reading and writing memory, injecting DLLs.

use std::ffi::c_void;
use std::io;
use std::path::Path;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, INVALID_HANDLE_VALUE, STILL_ACTIVE, WAIT_OBJECT_0};
use windows_sys::Win32::System::Diagnostics::Debug::{FlushInstructionCache, ReadProcessMemory, WriteProcessMemory};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, MODULEENTRY32W, Module32FirstW, Module32NextW, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPMODULE,
    TH32CS_SNAPMODULE32, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows_sys::Win32::System::Memory::{
    MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_EXECUTE_READWRITE, PAGE_READWRITE, VirtualAllocEx, VirtualFreeEx, VirtualProtectEx,
};
use windows_sys::Win32::System::Threading::{
    CreateRemoteThread, GetExitCodeProcess, GetExitCodeThread, OpenProcess, PROCESS_CREATE_THREAD, PROCESS_QUERY_INFORMATION, PROCESS_VM_OPERATION,
    PROCESS_VM_READ, PROCESS_VM_WRITE, WaitForSingleObject,
};

use crate::memory::{Memory, ReadError, ReadResult};
use crate::wide;

/// A running process.
#[derive(Debug, Clone)]
pub struct ProcessEntry {
    pub pid: u32,
    pub exe: String,
}

struct Snapshot(HANDLE);

impl Snapshot {
    fn new(flags: u32, pid: u32) -> io::Result<Snapshot> {
        let h = unsafe { CreateToolhelp32Snapshot(flags, pid) };
        if h == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Snapshot(h))
        }
    }
}

impl Drop for Snapshot {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

pub fn processes() -> io::Result<Vec<ProcessEntry>> {
    let snap = Snapshot::new(TH32CS_SNAPPROCESS, 0)?;
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<PROCESSENTRY32W>() as u32;
    let mut out = Vec::new();
    let mut ok = unsafe { Process32FirstW(snap.0, &mut entry) };
    while ok != 0 {
        out.push(ProcessEntry {
            pid: entry.th32ProcessID,
            exe: from_wide(&entry.szExeFile),
        });
        ok = unsafe { Process32NextW(snap.0, &mut entry) };
    }
    Ok(out)
}

/// Finds a process by executable name, case-insensitive.
pub fn find_process(exe: &str) -> Option<u32> {
    processes().ok()?.into_iter().find(|p| p.exe.eq_ignore_ascii_case(exe)).map(|p| p.pid)
}

#[derive(Debug, Clone)]
pub struct ModuleEntry {
    pub name: String,
    pub base: u64,
    pub size: u32,
    pub path: String,
}

pub fn modules(pid: u32) -> io::Result<Vec<ModuleEntry>> {
    let snap = Snapshot::new(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid)?;
    let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = size_of::<MODULEENTRY32W>() as u32;
    let mut out = Vec::new();
    let mut ok = unsafe { Module32FirstW(snap.0, &mut entry) };
    while ok != 0 {
        out.push(ModuleEntry {
            name: from_wide(&entry.szModule),
            base: entry.modBaseAddr as u64,
            size: entry.modBaseSize,
            path: from_wide(&entry.szExePath),
        });
        ok = unsafe { Module32NextW(snap.0, &mut entry) };
    }
    Ok(out)
}

/// An open process handle. Read-only opens only ask for `PROCESS_VM_READ | PROCESS_QUERY_INFORMATION`.
pub struct Process {
    handle: usize,
    pid: u32,
    writable: bool,
}

// A handle is just a kernel object number; using it across threads is safe
unsafe impl Send for Process {}
unsafe impl Sync for Process {}

impl Process {
    pub fn open(pid: u32) -> io::Result<Process> {
        Self::open_with(pid, false)
    }

    /// Access needed to write memory, change protection and create remote threads. Only used for injection and patching.
    pub fn open_writable(pid: u32) -> io::Result<Process> {
        Self::open_with(pid, true)
    }

    fn open_with(pid: u32, writable: bool) -> io::Result<Process> {
        let mut access = PROCESS_VM_READ | PROCESS_QUERY_INFORMATION;
        if writable {
            access |= PROCESS_VM_WRITE | PROCESS_VM_OPERATION | PROCESS_CREATE_THREAD;
        }
        let h = unsafe { OpenProcess(access, 0, pid) };
        if h.is_null() {
            return Err(io::Error::last_os_error());
        }
        Ok(Process {
            handle: h as usize,
            pid,
            writable,
        })
    }

    fn h(&self) -> HANDLE {
        self.handle as HANDLE
    }

    pub fn pid(&self) -> u32 {
        self.pid
    }

    pub fn is_alive(&self) -> bool {
        let mut code = 0u32;
        let ok = unsafe { GetExitCodeProcess(self.h(), &mut code) };
        ok != 0 && code == STILL_ACTIVE as u32
    }

    pub fn module(&self, name: &str) -> Option<ModuleEntry> {
        modules(self.pid).ok()?.into_iter().find(|m| m.name.eq_ignore_ascii_case(name))
    }

    pub fn write_bytes(&self, address: u64, bytes: &[u8]) -> io::Result<()> {
        self.require_writable()?;
        let mut written = 0usize;
        let ok = unsafe { WriteProcessMemory(self.h(), address as *const c_void, bytes.as_ptr() as *const c_void, bytes.len(), &mut written) };
        if ok == 0 || written != bytes.len() {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    /// Rewrites code: temporarily makes it writable and executable, then restores the old protection and flushes the instruction cache.
    pub fn patch_code(&self, address: u64, bytes: &[u8]) -> io::Result<()> {
        self.require_writable()?;
        let mut old = 0u32;
        let ok = unsafe { VirtualProtectEx(self.h(), address as *const c_void, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        let result = self.write_bytes(address, bytes);
        let mut ignored = 0u32;
        unsafe {
            VirtualProtectEx(self.h(), address as *const c_void, bytes.len(), old, &mut ignored);
            FlushInstructionCache(self.h(), address as *const c_void, bytes.len());
        }
        result
    }

    /// Loads a DLL into the target process with a `LoadLibraryW` remote thread and waits for it to return.
    pub fn inject_dll(&self, dll: &Path) -> io::Result<()> {
        self.require_writable()?;
        let path = wide(dll.as_os_str());
        let bytes: &[u8] = bytemuck::cast_slice(&path);

        let remote = unsafe { VirtualAllocEx(self.h(), std::ptr::null(), bytes.len(), MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE) };
        if remote.is_null() {
            return Err(io::Error::last_os_error());
        }
        let free = |p: *mut c_void| unsafe {
            VirtualFreeEx(self.h(), p, 0, MEM_RELEASE);
        };
        if let Err(e) = self.write_bytes(remote as u64, bytes) {
            free(remote);
            return Err(e);
        }

        let kernel32 = wide(std::ffi::OsStr::new("kernel32.dll"));
        let module = unsafe { GetModuleHandleW(kernel32.as_ptr()) };
        let load_library = unsafe { GetProcAddress(module, c"LoadLibraryW".as_ptr() as *const u8) };
        let Some(load_library) = load_library else {
            free(remote);
            return Err(io::Error::last_os_error());
        };

        let thread = unsafe {
            CreateRemoteThread(
                self.h(),
                std::ptr::null(),
                0,
                Some(std::mem::transmute::<
                    unsafe extern "system" fn() -> isize,
                    unsafe extern "system" fn(*mut c_void) -> u32,
                >(load_library)),
                remote,
                0,
                std::ptr::null_mut(),
            )
        };
        if thread.is_null() {
            let e = io::Error::last_os_error();
            free(remote);
            return Err(e);
        }
        let wait = unsafe { WaitForSingleObject(thread, 10_000) };
        let mut code = 0u32;
        unsafe {
            GetExitCodeThread(thread, &mut code);
            CloseHandle(thread);
        }
        free(remote);
        if wait != WAIT_OBJECT_0 {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "LoadLibraryW 超时"));
        }
        // The thread's exit code is the low 32 bits of the module handle; 0 means the load failed
        if code == 0 {
            return Err(io::Error::other("LoadLibraryW 返回 0"));
        }
        Ok(())
    }

    fn require_writable(&self) -> io::Result<()> {
        if self.writable {
            Ok(())
        } else {
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "进程是只读打开的"))
        }
    }
}

impl Memory for Process {
    fn read_bytes(&self, address: u64, buf: &mut [u8]) -> ReadResult<()> {
        if buf.is_empty() {
            return Ok(());
        }
        let mut read = 0usize;
        let ok = unsafe { ReadProcessMemory(self.h(), address as *const c_void, buf.as_mut_ptr() as *mut c_void, buf.len(), &mut read) };
        if ok == 0 || read != buf.len() {
            return Err(ReadError { address, len: buf.len() });
        }
        Ok(())
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.h()) };
    }
}

/// The last Win32 error code.
pub fn last_error() -> u32 {
    unsafe { GetLastError() }
}
