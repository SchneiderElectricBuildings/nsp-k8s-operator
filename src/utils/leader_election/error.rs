use kube::core::Status;

#[derive(thiserror::Error, Debug)]
pub enum LeaderError {
    #[error("could not get the Lease from Kubernetes: {0}")]
    GetLease(kube::Error),

    #[error("could not create the Lease in Kubernetes: {0}")]
    CreateLease(kube::Error),

    #[error("failed to acquire the Lease in Kubernetes: {0}")]
    AcquireLease(kube::Error),

    #[error("failed to renew the Lease in Kubernetes: {0}")]
    RenewLease(kube::Error),

    #[error("failed to traverse the Lease spec from Kubernetes at key `{key:}`")]
    TraverseLease { key: String },

    #[error("got unexpected Kubernetes API error: {response:}")]
    ApiError { response: Box<Status> },

    #[error("aborted to release lock because we are not leading, the lock is held by {leader:}")]
    ReleaseLockWhenNotLeading { leader: String },

    #[error("failed to release the lock in Kubernetes: {0}")]
    ReleaseLease(kube::Error),

    #[error("error deserializing response")]
    SerdeError(#[from] serde_json::Error),

    #[error(transparent)]
    Kube(#[from] kube::Error),
}
