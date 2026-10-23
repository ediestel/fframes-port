use std::ops::Deref;

#[repr(transparent)]
pub struct LumaPlane(pub *mut u8);

#[repr(transparent)]
pub struct ChromaPlane(pub *mut u8);

#[repr(transparent)]
pub struct AlphaPlane(pub *mut u8);

impl Deref for LumaPlane {
    type Target = *mut u8;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for ChromaPlane {
    type Target = *mut u8;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for AlphaPlane {
    type Target = *mut u8;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
