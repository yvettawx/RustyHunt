//! 读另一个进程内存的统一接口。真实进程和测试用的假内存都实现 [`Memory`]。

use std::collections::BTreeMap;
use std::fmt;

use bytemuck::Pod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadError {
    pub address: u64,
    pub len: usize,
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "读取 0x{:X} 处 {} 字节失败", self.address, self.len)
    }
}

impl std::error::Error for ReadError {}

pub type ReadResult<T> = Result<T, ReadError>;

pub trait Memory {
    /// 把 `address` 开始的 `buf.len()` 个字节读进 `buf`；读不全算失败。
    fn read_bytes(&self, address: u64, buf: &mut [u8]) -> ReadResult<()>;

    fn read<T: Pod>(&self, address: u64) -> ReadResult<T>
    where
        Self: Sized,
    {
        let mut value = T::zeroed();
        self.read_bytes(address, bytemuck::bytes_of_mut(&mut value))?;
        Ok(value)
    }

    fn read_vec<T: Pod>(&self, address: u64, count: usize) -> ReadResult<Vec<T>>
    where
        Self: Sized,
    {
        let mut values = vec![T::zeroed(); count];
        if count > 0 {
            self.read_bytes(address, bytemuck::cast_slice_mut(&mut values))?;
        }
        Ok(values)
    }

    fn read_ptr(&self, address: u64) -> ReadResult<u64>
    where
        Self: Sized,
    {
        self.read::<u64>(address)
    }

    /// 读一个以 0 结尾的 UTF-8 字符串，最多 `max_len` 字节。
    fn read_string(&self, address: u64, max_len: usize) -> ReadResult<String>
    where
        Self: Sized,
    {
        let mut buf = vec![0u8; max_len];
        self.read_bytes(address, &mut buf)?;
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        Ok(String::from_utf8_lossy(&buf[..end]).into_owned())
    }

    /// 上游 `ReadAsync(address, offsets)`：每一步先解引用当前地址，再加偏移。
    /// 中途读到空指针时返回 0。
    fn follow(&self, address: u64, offsets: &[i32]) -> ReadResult<u64>
    where
        Self: Sized,
    {
        let mut address = address;
        for &offset in offsets {
            let next = self.read_ptr(address)?;
            if next == 0 {
                return Ok(0);
            }
            address = next.wrapping_add_signed(offset as i64);
        }
        Ok(address)
    }

    /// 上游 `ReadPtrAsync(address, offsets)`：每一步先加偏移，再解引用。
    fn follow_ptr(&self, address: u64, offsets: &[i32]) -> ReadResult<u64>
    where
        Self: Sized,
    {
        let mut address = address;
        for &offset in offsets {
            let next = self.read_ptr(address.wrapping_add_signed(offset as i64))?;
            if next == 0 {
                return Ok(0);
            }
            address = next;
        }
        Ok(address)
    }

    /// 上游 `DerefAsync<T>`：按 [`Memory::follow`] 走完指针链再读值；链断了得到零值。
    fn deref<T: Pod>(&self, address: u64, offsets: &[i32]) -> ReadResult<T>
    where
        Self: Sized,
    {
        match self.follow(address, offsets)? {
            0 => Ok(T::zeroed()),
            target => self.read(target),
        }
    }
}

impl<M: Memory + ?Sized> Memory for &M {
    fn read_bytes(&self, address: u64, buf: &mut [u8]) -> ReadResult<()> {
        (**self).read_bytes(address, buf)
    }
}

/// 测试用的内存：若干段按地址存放的字节。
#[derive(Debug, Default, Clone)]
pub struct FakeMemory {
    regions: BTreeMap<u64, Vec<u8>>,
}

impl FakeMemory {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn put_bytes(&mut self, address: u64, bytes: &[u8]) {
        // 和已有的段重叠时直接写进去，否则新开一段
        for (&start, region) in self.regions.iter_mut() {
            let end = start + region.len() as u64;
            if address >= start && address + bytes.len() as u64 <= end {
                let at = (address - start) as usize;
                region[at..at + bytes.len()].copy_from_slice(bytes);
                return;
            }
        }
        self.regions.insert(address, bytes.to_vec());
    }

    pub fn put<T: bytemuck::NoUninit>(&mut self, address: u64, value: T) {
        self.put_bytes(address, bytemuck::bytes_of(&value));
    }

    pub fn put_slice<T: bytemuck::NoUninit>(&mut self, address: u64, values: &[T]) {
        self.put_bytes(address, bytemuck::cast_slice(values));
    }

    /// 预留一段全 0 的内存，之后的 `put` 会写进这一段。
    pub fn reserve(&mut self, address: u64, len: usize) {
        self.regions.insert(address, vec![0; len]);
    }
}

impl Memory for FakeMemory {
    fn read_bytes(&self, address: u64, buf: &mut [u8]) -> ReadResult<()> {
        let err = ReadError { address, len: buf.len() };
        let (&start, region) = self.regions.range(..=address).next_back().ok_or(err)?;
        let at = (address - start) as usize;
        let src = region.get(at..at + buf.len()).ok_or(err)?;
        buf.copy_from_slice(src);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_dereferences_then_adds() {
        let mut m = FakeMemory::new();
        m.put(0x1000u64, 0x2000u64);
        m.put(0x2010u64, 0x3000u64);
        m.put(0x3008u64, 42i32);
        assert_eq!(m.follow(0x1000, &[0x10, 0x8]).unwrap(), 0x3008);
        assert_eq!(m.deref::<i32>(0x1000, &[0x10, 0x8]).unwrap(), 42);
    }

    #[test]
    fn follow_stops_on_null() {
        let mut m = FakeMemory::new();
        m.put(0x1000u64, 0u64);
        assert_eq!(m.follow(0x1000, &[0x10, 0x8]).unwrap(), 0);
        assert_eq!(m.deref::<i32>(0x1000, &[0x10]).unwrap(), 0);
    }

    #[test]
    fn follow_ptr_adds_then_dereferences() {
        let mut m = FakeMemory::new();
        m.put(0x1010u64, 0x2000u64);
        m.put(0x2008u64, 0x3000u64);
        assert_eq!(m.follow_ptr(0x1000, &[0x10, 0x8]).unwrap(), 0x3000);
    }

    #[test]
    fn negative_offsets_work() {
        let mut m = FakeMemory::new();
        m.put(0x1000u64, 0x2010u64);
        assert_eq!(m.follow(0x1000, &[-0x10]).unwrap(), 0x2000);
    }

    #[test]
    fn read_string_stops_at_nul() {
        let mut m = FakeMemory::new();
        m.put_bytes(0x10, b"em\\em001\0garbage");
        assert_eq!(m.read_string(0x10, 16).unwrap(), "em\\em001");
    }

    #[test]
    fn unmapped_read_fails() {
        let m = FakeMemory::new();
        assert!(m.read::<u32>(0x10).is_err());
    }

    #[test]
    fn put_inside_reserved_region() {
        let mut m = FakeMemory::new();
        m.reserve(0x100, 0x100);
        m.put(0x180u32 as u64, 7u32);
        assert_eq!(m.read::<u32>(0x180).unwrap(), 7);
        assert_eq!(m.read::<u32>(0x100).unwrap(), 0);
    }
}
