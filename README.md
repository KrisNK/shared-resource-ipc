# Shared Resource IPC

Wrapper for a resource that can be shared across processes.

### Supported Operating Systems
- [x] Linux
- [x] MacOS
- [ ] Windows

Linux/MacOS support is achieved using POSIX shared memory and semaphores.

Windows support is desired, but not planned any time soon.

### Usage

In your `Cargo.toml` file, add the line:
```toml
shared-resource-ipc = "0.1"
```

Here is a basic example of what can be done with this package.

```rust
use shared_resource_ipc::SharedResource;
use serde::{Serialize, Deserialize};

#[derive(Clone, Default, Serialize, Deserialize, Default)]
struct DemoStruct {
    number: usize,
    name: String,
}

fn main() {
    let default_value: DemoStruct = DemoStruct::default();

    // Open a shared resource using a unique identifier and a default value. If this process
    // is not the first to initialize the resource, the default value is ignored.
    let shared_resource: SharedResource<DemoStruct> = 
        SharedResource::new("unique_name", default_value)
        .expect("failed to open shared resource");

    // Access the stored value using a clojure.
    let _result = shared_resource.access(|data: &DemoStruct| {
        // read the data here...
        let value = data.clone();
        // a value can be returned from the clojure
        return value;
    }).expect("failed to access shared resource");

    // The value can be accessed mutably as well.
    let _result = shared_resource.access(|data| {
        (*data).number = 42;
    }).expect("failed to mutably access shared resource");

    // No cleaning is required: it is handled when the resource is dropped.
}
```

### Possible Issue

This implementation of a shared resource does not know how many processes connect to the memory segment.

This means that sometimes, a process will create, open and completely destroy the memory before another process has the change to access it. The program will panic **most times** when this happens. However, it won't panic if a second process re creates the memory not knowing it was already destroyed before. As far as that second process is concerned, it is the first one to create the resource so it will not panic.

**Solution**: make sure your resource has a certain *goal* to accomplish. As such, a condition can be tested to then drop the resource in a process.

Alternatively, a delay can be added before the resource drops, but this will work less predictably.

If you know of any way to solve this problem, please open an issue.

