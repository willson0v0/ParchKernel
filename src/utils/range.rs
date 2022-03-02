use core::fmt::Debug;

pub trait StepUp {
    fn step_up(&mut self);
}

pub trait StepDown {
    fn step_down(&mut self);
}

#[derive(Copy, Clone)]
pub struct Range<T> where 
    T: StepUp + StepDown + Copy + PartialEq + PartialOrd + Debug, {
    start: T,
    end: T,
}

impl <T> Range<T> where T: StepUp + StepDown + Copy + PartialEq + PartialOrd + Debug {
    pub fn new(start: T, end: T) -> Self {
        Self {start, end}
    }
    pub fn start(&self) -> T {self.start}
    pub fn end(&self) -> T {self.end}
    pub fn contains(&self, tgt: T) -> bool {
        tgt >= self.start && tgt < self.end
    }
}

pub struct RangeIterator<T> where T: StepUp + StepDown + Copy + PartialEq + PartialOrd + Debug {
    current: T,
    end: T,
}

impl<T> RangeIterator<T> where 
    T: StepUp + StepDown + Copy + PartialEq + PartialOrd + Debug {
    pub fn new(current: T, end: T) -> Self{
        Self {current, end}
    }
}

impl<T> Iterator for RangeIterator<T> where 
    T: StepUp + StepDown + Copy + PartialEq + PartialOrd + Debug {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current == self.end {
            None
        } else {
            let t = self.current;
            if self.current < self.end {
                self.current.step_up();
            } else {
                self.current.step_down();
            }
            Some(t)
        }
    }
}
    

impl<T> IntoIterator for Range<T> where 
    T: StepUp + StepDown + Copy + PartialEq + PartialOrd + Debug {
    type Item = T;
    type IntoIter = RangeIterator<T>;

    fn into_iter(self) -> Self::IntoIter {
        RangeIterator::new(self.start, self.end)
    }
}
