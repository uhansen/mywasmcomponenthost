use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker, Val};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiView};

/// Host state that provides WASI capabilities to the component
struct HostState {
    wasi_ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for HostState {
    fn ctx(&mut self) -> &mut WasiCtx {
        &mut self.wasi_ctx
    }

    fn table(&mut self) -> &mut ResourceTable {
        &mut self.table
    }
}

fn main() -> Result<()> {
    // Create an engine with the component model enabled
    let mut config = Config::new();
    config.wasm_component_model(true);
    let engine = Engine::new(&config).context("Failed to create Wasmtime engine")?;

    
    // Load the WASM component
    let wasm_path = "/home/uffe/source/projects/myhomemonitor/getCoordsFromAddress/target/wasm32-wasip2/release/getCoordsFromAddress.wasm";
    println!("Loading WASM component from: {}", wasm_path);
    let component = Component::from_file(&engine, wasm_path)
        .context("Failed to load WASM component")?;

    // Create a linker and add WASI support
    let mut linker: Linker<HostState> = Linker::new(&engine);
    wasmtime_wasi::add_to_linker_sync(&mut linker)
        .context("Failed to add WASI to linker")?;

    // Get the geocoding API key from environment
    let geocoding_api_key = std::env::var("GEOCODING_API_KEY")
        .unwrap_or_else(|_| "".to_string());

    // Create WASI context with environment access
    // Expose the API key to the component via wasi:environment interface
    let wasi_ctx = WasiCtxBuilder::new()
        .env("GEOCODING_API_KEY", &geocoding_api_key)
        .inherit_stderr()
        .inherit_stdout()
        .build();

    // Create a store with the host state
    let mut store = Store::new(&engine, HostState {
        wasi_ctx,
        table: ResourceTable::new(),
    });

    // Instantiate the component
    let instance = linker
        .instantiate(&mut store, &component)
        .context("Failed to instantiate WASM component")?;

    // List available exports to find the correct name
    println!("\nAvailable exports:");
    let component_type = component.component_type();
    for (name, _) in component_type.exports(&engine) {
        println!("  - {}", name);
    }

    // Get the exported interface index first
    let interface_idx = instance
        .get_export(&mut store, None, "docs:getcoordsfromaddressworld/getcoordsfromaddress@0.1.0")
        .context("Failed to find exported interface")?;
    
    // Now get the function from within the interface
    let func_idx = instance
        .get_export(&mut store, Some(&interface_idx), "getcoordsfromaddress")
        .context("Failed to find getcoordsfromaddress export within interface")?;
    
    // Get the actual function
    let func = instance
        .get_func(&mut store, func_idx)
        .context("Failed to get function from export")?;

    // Create the address record as a component value
    // Address record: { street, streetnumber, zip, town, region, count }
    let address = Val::Record(vec![
        ("street".to_string(), Val::String("Amagertorv".into())),
        ("streetnumber".to_string(), Val::String("1".into())),
        ("zip".to_string(), Val::String("1160".into())),
        ("town".to_string(), Val::String("København K".into())),
        ("region".to_string(), Val::Option(Some(Box::new(Val::String("Denmark".into()))))),
        ("count".to_string(), Val::Option(None)),
    ]);

    println!("\nLooking up coordinates for:");
    println!("  Street: Amagertorv 1");
    println!("  Zip: 1160");
    println!("  Town: København K");
    println!("  Region: Denmark");

    // Call the function
    let mut results = vec![Val::Bool(false)]; // Placeholder for result
    func.call(&mut store, &[address], &mut results)
        .context("Failed to call getcoordsfromaddress")?;
    func.post_return(&mut store)
        .context("Failed to complete post-return")?;

    // Extract coordinates from the result
    if let Val::Record(fields) = &results[0] {
        let mut latitude = 42.0;
        let mut longitude = 42.0;
        
        for (name, val) in fields {
            match (name.as_str(), val) {
                ("latitude", Val::Float64(v)) => latitude = *v,
                ("longitude", Val::Float64(v)) => longitude = *v,
                _ => {}
            }
        }
        
        println!("\nCoordinates:");
        println!("  Latitude:  {}", latitude);
        println!("  Longitude: {}", longitude);
    } else {
        println!("Unexpected result type: {:?}", results[0]);
    }

    Ok(())
}
