# Kubernetes operator for EboServer resources

This Kubernetes operator lets you deploy EBO server containers as a custom resource.

## Build instructions

### Build statically linked executable

```shell
apt-get update && apt-get install musl-tools musl-dev
rustup target add x86_64-unknown-linux-musl
cargo build --target  x86_64-unknown-linux-musl
```

### Build container image

```shell
docker build -t nsp-k8s-operator .
```

## Deployment

### Using Helm

The operator can be deployed using the provided Helm chart.

```shell
helm install nsp-k8s-operator ./charts/ebo-server-k8s-op -n ebo-operator --create-namespace
```

#### Configuration

You can customize the deployment by creating a `values.yaml` file or by passing parameters via the `--set` flag. For example, to set the log level and the namespace scope:

```shell
helm install nsp-k8s-operator ./charts/ebo-server-k8s-op \
  --set logLevel=debug \
  --set config.namespaceScope=my-watched-namespace \
  -n ebo-operator --create-namespace
```

Refer to [charts/ebo-server-k8s-op/values.yaml](charts/ebo-server-k8s-op/values.yaml) for a full list of available configuration options.

## Usage

Once you have the operator running (for a given namespace), simply create an EboServer resource
definition in yaml (`example-es.yaml`)...

```yaml
apiVersion: digbld.em.se.com/v1alpha6
kind: EboServer
metadata:
  name: example-es
spec:
  type: EnterpriseServer
  version: 7.1.1.132
```

and deploy it...

```shell
kubectl apply -n my-namespace -f example-cs.yaml
```

In just a moment you will have a pvc, svc and pod created and running...

```shell
NAME                                      STATUS   VOLUME                                     CAPACITY   ACCESS MODES   STORAGECLASS   VOLUMEATTRIBUTESCLASS   AGE
persistentvolumeclaim/example-es-backup   Bound    pvc-32153578-ad64-40c5-92bc-c864ae0087c1   4Gi        RWO            standard       <unset>                 20m
persistentvolumeclaim/example-es-data     Bound    pvc-39ad71c0-8d62-46c5-8cdb-f7ede07e132c   4Gi        RWO            standard       <unset>                 20m

NAME                 TYPE       CLUSTER-IP    EXTERNAL-IP   PORT(S)                                                    AGE
service/example-es   NodePort   10.96.45.67   <none>        80:30699/TCP,443:31090/TCP,4444:32753/TCP,8080:31855/TCP   20m

NAME             READY   STATUS    RESTARTS   AGE
pod/example-es   1/1     Running   0          3m31s
```
