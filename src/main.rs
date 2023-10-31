use std::sync::Arc;
use yang2::schema::SchemaNodeKind;
use yang2::context::{Context, ContextFlags};

enum Mode {
    Convert(ConvertMode),
}

enum ConvertMode {
    Nix2Yang,
    Yang2Nix,
}

fn main() -> std::io::Result<()> {

    let mut args = std::env::args();

    drop(args.next());

    let mode = match args.next().as_ref().map(|x| x.as_str()) {
        Some("yang2nix") => Mode::Convert(ConvertMode::Yang2Nix),
        Some("nix2yang") => Mode::Convert(ConvertMode::Nix2Yang),
        _ => panic!("mode: yang2nix nix2yang"),
    };

    // Initialize context.
    let mut ctx = Context::new(ContextFlags::NO_YANGLIBRARY)
        .expect("Failed to create context");
    ctx.set_searchdir(std::env::var("YANG_SCHEMAS_DIR").expect("env var YANG_SCHEMAS_DIR"))
        .expect("Failed to set YANG search directory");
    
    ctx.load_module("rtbrick-config", None, &[])
        .expect("Failed to load module");

    let ctx = Arc::new(ctx);

    let module = ctx.get_module_latest("rtbrick-config").unwrap();

    let roots = module.data();

    let mode = match mode {
        Mode::Convert(mode) => mode,
    };

    let file = args.next().expect("filename");

    let mut data: serde_json::Value = serde_json::from_slice(&std::fs::read(file)?).unwrap();

    for node in roots.flat_map(|root| root.traverse())
        // only lists that have keys
        .filter(|node| node.kind() == SchemaNodeKind::List && !node.is_keyless_list())
    {
        let key_names = node.list_keys().map(|ch| format!("{}", ch.name())).collect::<Vec<_>>();
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
                        ConvertMode::Yang2Nix => {
                            let as_array = if let serde_json::Value::Array(a) = e.take() {
                                a
                            } else { panic!("Expected an array. Are you sure this is a YANG-style file?") };

                            for mut el in as_array {
                                let mut p2 = &mut *e; // reference to the value where the element will be inserted
                                for key in &key_names {
                                    let k = el.as_object_mut().expect("expected an object").remove(key).expect("expected key");
                                    let k = k.as_str().map(|s| s.to_string()).or(k.as_number().and_then(|n| n.as_i64()).map(|n| n.to_string())).expect("can not determine key");

                                    if !p2.is_object() { *p2 = serde_json::Value::Object(Default::default()); };
                                    p2 = p2.as_object_mut().unwrap().entry(k).or_insert(serde_json::Value::Null);
                                }
                                *p2 = el; // insert element
                            }
                        }
                        ConvertMode::Nix2Yang => {

                            let mut a = vec![];

                            let mut q: Vec<(Vec<String>, _)> = vec![(vec![], e.take())];

                            while let Some((depth, mut el)) = q.pop() {
                                if depth.len() == key_names.len() {
                                    for (key, key_name) in depth.into_iter().zip(&key_names) {
                                        el.as_object_mut().expect("expected object").insert(key_name.clone(), key.into());
                                    }
                                    a.push(el);
                                } else {
                                    //println!("{:?}", depth);
                                    //println!("{:?}", el);
                                    let as_object = if let serde_json::Value::Object(o) = el.take() {
                                        o
                                    } else { panic!("Expected an object. Are you sure this is a Nix-style file?"); };
                                    for (key, el2) in as_object {
                                        let mut depth = depth.clone();
                                        depth.push(key);
                                        q.push((depth, el2));
                                    }
                                }
                            }

                            *e = serde_json::Value::Array(a);
                        }
                    }

                    //println!("{:?}", e);
                }
                //println!("");
                break;
            }
            if an.kind() == SchemaNodeKind::List {
                for _ in an.list_keys() {
                    p = p.into_iter().flat_map(|x| -> Box<dyn Iterator<Item = &mut serde_json::Value>> {
                        match x {
                            serde_json::Value::Array(a) => Box::new(a.into_iter()),
                            serde_json::Value::Object(o) => Box::new(o.values_mut()),
                            _ => panic!(),
                        }
                    }).collect();
                }
            }

            if p.len() == 0 { break; }
        }

    }

    println!("{}", serde_json::to_string(&data).unwrap());

    Ok(())
}
