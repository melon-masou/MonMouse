use std::time::{Duration, Instant, SystemTime};

pub struct SimpleRatelimit {
    next: Instant,
    once_per: Duration,
}

impl SimpleRatelimit {
    pub fn new(once_per: Duration, init: Option<Instant>) -> SimpleRatelimit {
        SimpleRatelimit {
            next: init.unwrap_or(Instant::now()),
            once_per,
        }
    }
    pub fn allow(&mut self, now: Option<Instant>) -> (bool, Duration) {
        let now = now.unwrap_or(Instant::now());
        if now >= self.next {
            self.next = now + self.once_per;
            (true, self.once_per)
        } else {
            (false, self.next - now)
        }
    }
    pub fn reset(&mut self, v: Duration) {
        self.next -= self.once_per;
        self.next = self
            .next
            .checked_sub(self.once_per)
            .unwrap_or(self.next - v);
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
    static mut _DELAY_PANIC_LAST: Option<SystemTime> = None;

    let now = SystemTime::now();
    if let Some(t) = unsafe { _DELAY_PANIC_LAST } {
        if now.duration_since(t).unwrap() > Duration::from_secs(seconds) {
            panic!("delay panic");
        }
    } else {
        unsafe { _DELAY_PANIC_LAST = Some(now) };
    }
}
