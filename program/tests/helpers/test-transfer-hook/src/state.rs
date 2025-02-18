use bytemuck::Pod;
use bytemuck::Zeroable;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
pub struct Counter {
    pub count: u64,
}
