use controller::crd::v1alpha6::EboServer;
use kube::CustomResourceExt;

fn main() {
    print!("{}", serde_yaml::to_string(&EboServer::crd()).unwrap())
}
