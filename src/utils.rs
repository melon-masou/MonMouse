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
}
