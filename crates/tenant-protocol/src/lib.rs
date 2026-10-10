//! Public API and private storage wire contracts. No application or database dependencies.
pub mod arena360 {
    pub mod v1 {
        tonic::include_proto!("arena360.v1");
    }
}

pub mod storage {
    tonic::include_proto!("arena360.storage.v1");
}
