use candle_core::{DType, Device, Tensor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum DTypeInner {
    // Unsigned 8 bits integer.
    U8,
    // Unsigned 32 bits integer.
    U32,
    // Signed 64 bits integer.
    I64,
    // Brain floating-point using half precision (16 bits).
    BF16,
    // Floating-point using half precision (16 bits).
    F16,
    // Floating-point using single precision (32 bits).
    F32,
    // Floating-point using double precision (64 bits).
    F64,
}

impl From<DType> for DTypeInner {
    fn from(value: DType) -> Self {
        match value {
            DType::U8 => Self::U8,
            DType::U32 => Self::U32,
            DType::I64 => Self::I64,
            DType::BF16 => Self::BF16,
            DType::F16 => Self::F16,
            DType::F32 => Self::F32,
            DType::F64 => Self::F64,
        }
    }
}

impl From<DTypeInner> for DType {
    fn from(value: DTypeInner) -> Self {
        match value {
            DTypeInner::U8 => Self::U8,
            DTypeInner::U32 => Self::U32,
            DTypeInner::I64 => Self::I64,
            DTypeInner::BF16 => Self::BF16,
            DTypeInner::F16 => Self::F16,
            DTypeInner::F32 => Self::F32,
            DTypeInner::F64 => Self::F64,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TensorBuf {
    dims: Vec<usize>,
    buf: Vec<u8>,
    dtype: DTypeInner,
}

impl From<Tensor> for TensorBuf {
    fn from(value: Tensor) -> Self {
        let mut buf = Vec::new();
        value.write_bytes(&mut buf).expect("Should write to buf");
        Self {
            dims: value.shape().dims().to_vec(),
            buf,
            dtype: value.dtype().into(),
        }
    }
}

impl TryFrom<Vec<u8>> for TensorBuf {
    type Error = bincode::Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        let tensor_buf = bincode::deserialize(&value)?;
        Ok(tensor_buf)
    }
}

impl TensorBuf {
    pub fn to_tensor(&self, device: &Device) -> Result<Tensor, candle_core::Error> {
        Tensor::from_raw_buffer(&self.buf, self.dtype.into(), &self.dims, &device)
    }

    pub fn to_vec(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap()
    }
}
