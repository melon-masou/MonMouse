pub struct SimpleRatelimit {
    next: u64,
    once_per: u64,
}

impl SimpleRatelimit {
    pub fn new(once_per: u64) -> SimpleRatelimit {
        SimpleRatelimit { next: 0, once_per }
    }
    pub fn allow(&mut self, tick: u64) -> bool {
        if tick >= self.next {
            self.next = tick + self.once_per;
            true
        } else {
            false
        }
    }
    pub fn reset(&mut self, v: u64) {
        self.next = self.next - self.once_per + v;
        self.once_per = v;
    }
}

pub struct ArrayVec<T: Copy, const N: usize> {
    arr: [Option<T>; N],
}

impl<T: Copy, const N: usize> Default for ArrayVec<T, N> {
    fn default() -> Self {
        Self { arr: [None; N] }
    }
}

impl<T: Copy, const N: usize> ArrayVec<T, N> {
    pub fn to_vec(&self) -> Vec<Option<T>> {
        self.arr.to_vec()
    }
}

pub fn delay_panic(seconds: u64) {
    use std::time::{Duration, SystemTime};
    static mut _DELAY_PANIC_LAST: Option<std::time::SystemTime> = None;

    let now = SystemTime::now();
    if let Some(t) = unsafe { _DELAY_PANIC_LAST } {
        if now.duration_since(t).unwrap() > Duration::from_secs(seconds) {
            panic!("delay panic");
        }
    } else {
        unsafe { _DELAY_PANIC_LAST = Some(now) };
    }
}
