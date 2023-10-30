use std::sync::Arc;
use std::fs::File;
use yang2::schema::SchemaNodeKind;
use yang2::schema::SchemaNode;
use yang2::schema::SchemaPathFormat;
use yang2::context::{Context, ContextFlags};
use yang2::data::{
    Data, DataDiffFlags, DataFormat, DataParserFlags, DataPrinterFlags,
    DataTree, DataValidationFlags,
};

enum Mode {
    Nix2Yang,
    Yang2Nix,
}

fn main() -> std::io::Result<()> {

    let mut args = std::env::args();

    drop(args.next());

    let mode = match args.next().as_ref().map(|x| x.as_str()) {
        Some("yang2nix") => Mode::Yang2Nix,
        Some("nix2yang") => Mode::Nix2Yang,
        _ => panic!("mode: yang2nix nix2yang"),
    };

    let file = args.next().expect("filename");

    let mut data: serde_json::Value = serde_json::from_slice(&std::fs::read(file)?).unwrap();

    // Initialize context.
    let mut ctx = Context::new(ContextFlags::NO_YANGLIBRARY)
        .expect("Failed to create context");
    ctx.set_searchdir(std::env::var("YANG_SCHEMAS_DIR").expect("env var YANG_SCHEMAS_DIR"))
        .expect("Failed to set YANG search directory");
    
    let module = ctx.load_module("rtbrick-config", None, &[])
        .expect("Failed to load module");

    let ctx = Arc::new(ctx);

    let module = ctx.get_module_latest("rtbrick-config").unwrap();

    // Parse data trees from JSON strings.
    let dtree1 = DataTree::parse_file(
        &ctx,
        File::open("./settings.json")?,
        DataFormat::JSON,
        DataParserFlags::NO_VALIDATION,
        DataValidationFlags::empty(),
    )
    .expect("Failed to parse data tree");

    for (node, key) in module.traverse()
        // only lists that have keys
        .filter(|node| node.kind() == SchemaNodeKind::List && !node.is_keyless_list())
        // only lists that have _exactly_ one key; and extract that key
        .filter_map(|node| {
            let keys = node.children().filter(SchemaNode::is_list_key).map(|ch| format!("{}", ch.name())).collect::<Vec<_>>();
            (keys.len() == 1).then(|| (node, keys.into_iter().next().unwrap()) )
        })
    {
        let mut p = vec![&mut data];

        let mut ancestors = node.inclusive_ancestors().collect::<Vec<_>>().into_iter().rev().enumerate();
        let ancestors_len = ancestors.len();

        for (i, an) in &mut ancestors {
            let k = if i == 0 {
                format!("rtbrick-config:{}", an.name())
            } else {
                format!("{}", an.name())
            };

            p = p.into_iter().flat_map(|x| x.get_mut(&k).into_iter()).collect();

            // last ancestor
            if i == (ancestors_len - 1) {
                // we didn't find anything
                if p.len() == 0 { break; }

                // last node ; convert
                //println!("{:?} {:?}", node.path(SchemaPathFormat::DATA), key);
                for e in p {

                    match mode {
                        Mode::Yang2Nix => {
                            let as_array = if let serde_json::Value::Array(a) = e.take() {
                                a
                            } else { panic!("Expected an array. Are you sure this is a YANG-style file?") };
                            let o = serde_json::Value::Object(
                                as_array.into_iter().map(|mut el| {
                                    let k = el.as_object_mut().expect("expected an object").remove(&key).expect("expected key");
                                    (
                                        k.as_str().map(|s| s.to_string()).or(k.as_number().and_then(|n| n.as_i64()).map(|n| n.to_string())).expect("can not determine key"),
                                        el
                                    )
                                }).collect::<serde_json::Map<String, serde_json::Value>>()
                            );
                            *e = o;
                        }
                        Mode::Nix2Yang => {
                            let as_object = if let serde_json::Value::Object(o) = e.take() {
                                o
                            } else { panic!("Expected an object. Are you sure this is a Nix-style file?") };
                            let a = serde_json::Value::Array(
                                as_object.into_iter().map(|(k, mut el)| {
                                    let el2 = el.as_object_mut().expect("expected an object");
                                    el2.insert(key.clone(), k.into());
                                    el
                                }).collect::<Vec<serde_json::Value>>()
                            );
                            *e = a;
                        }
                    }

                    //println!("{:?}", e);
                }
                //println!("");
                break;
            }
            if an.kind() == SchemaNodeKind::List {
                p = p.into_iter().flat_map(|x| -> Box<dyn Iterator<Item = &mut serde_json::Value>> {
                    match x {
                        serde_json::Value::Array(a) => Box::new(a.into_iter()),
                        serde_json::Value::Object(o) => Box::new(o.values_mut()),
                        _ => panic!(),
                    }
                }).collect();
            }

            if p.len() == 0 { break; }
        }

    }

    println!("{}", serde_json::to_string(&data).unwrap());

    Ok(())
}
