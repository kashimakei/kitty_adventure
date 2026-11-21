#[cfg(feature = "python")]
use pyo3::prelude::*;

#[cfg_attr(feature = "python", pyclass(get_all, set_all))]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

#[cfg_attr(feature = "python", pymethods)]
impl Vec2 {
    // Mathematical operations exposed to Python
    pub fn add(&self, other: &Vec2) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }

    pub fn sub(&self, other: &Vec2) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    pub fn mul(&self, scalar: f32) -> Self {
        Self::new(self.x * scalar, self.y * scalar)
    }

    pub fn dot(&self, other: &Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }
    
    pub fn length_sq(&self) -> f32 {
        self.dot(self)
    }
    
    pub fn length(&self) -> f32 {
        self.length_sq().sqrt()
    }
    
    pub fn normalize(&self) -> Self {
        let len = self.length();
        if len > 0.0 {
            self.mul(1.0 / len)
        } else {
            Self::zero()
        }
    }
}

// Inner Rust versions for ergonomic matching with older code
impl Vec2 {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self::new(0.0, 0.0)
    }

    pub fn add_val(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
    pub fn sub_val(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
    pub fn mul_val(self, scalar: f32) -> Self {
        Self::new(self.x * scalar, self.y * scalar)
    }
    pub fn dot_val(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y
    }
    pub fn length_sq_val(self) -> f32 {
        self.dot_val(self)
    }
    pub fn length_val(self) -> f32 {
        self.length_sq_val().sqrt()
    }
    pub fn normalize_val(self) -> Self {
        let len = self.length_val();
        if len > 0.0 {
            self.mul_val(1.0 / len)
        } else {
            Self::zero()
        }
    }
}

/// Python constructors live apart so the type builds without PyO3 (the `python` feature).
#[cfg(feature = "python")]
#[pymethods]
impl Vec2 {
    #[new]
    fn py_new(x: f32, y: f32) -> Self {
        Self::new(x, y)
    }

    #[staticmethod]
    #[pyo3(name = "zero")]
    fn py_zero() -> Self {
        Self::zero()
    }
}
