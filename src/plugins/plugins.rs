use wasmtime::component::{Component, Linker, bindgen};
use wasmtime::{Engine, Store};

bindgen!("plugin");

fn main() -> Result<(),()> {
    let engine = Engine::default();
    let component = match Component::from_file(&engine, "guest.wasm") {
		Ok(lol) => lol,
		Err(e) => {
			println!("{}", e);
			panic!("Fuck you");
		}
	};
    let linker = Linker::new(&engine);
    let mut store = Store::new(&engine, ());
    let instance = match Plugin::instantiate(&mut store, &component, &linker) {
		Ok(x) => x,
		Err(_y) => panic!("Fcuk")
	};

	let x = instance.interface0.call_baz(&mut store);
    Ok(())
}

