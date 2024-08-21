use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use yang2::context::{Context, ContextFlags};
use yang2::schema::DataValueType;
use yang2::schema::SchemaNode;
use yang2::schema::SchemaNodeKind;

enum Mode {
    NixOptions,
    Convert(ConvertMode, File),
    Diff(File, File),
}

enum ConvertMode {
    Nix2Yang,
    Yang2Nix,
}

fn print_nix_options(indent: &mut String, root: SchemaNode) {
    let mut stack = vec![root];

    while let Some(node) = stack.pop() {
        //println!("{}}}", indent);
        match node.kind() {
            SchemaNodeKind::Container => {
                if let Some(description) = node.description() {
                    println!("\n{}# {}", indent, description);
                }
                println!("{}{} = {{", indent, node.name());
                *indent += "  ";
                for child in node.children() {
                    print_nix_options(indent, child);
                }
                *indent = indent.chars().skip(2).collect();
                println!("{}}};", indent);
            }

            SchemaNodeKind::List => {
                println!("\n{}{} = lib.mkOption {{", indent, node.name());
                *indent += "  ";

                println!("{}description = ''", indent);
                if let Some(description) = node.description() {
                    println!("{}  {}", indent, description);
                }
                for (i, key) in node.list_keys().enumerate() {
                    println!("{}  Key {}: {}", indent, i + 1, key.name());
                }
                println!("{}'';", indent);

                print!("{}type = ", indent);
                for key in node.list_keys() {
                    print!("lib.types.attrsOf (");
                }
                println!("lib.types.submodule {{\n");
                *indent += "  ";
                println!("{}options = {{", indent);
                *indent += "  ";

                for child in node.children() {
                    if !child.is_list_key() {
                        print_nix_options(indent, child);
                    }
                }

                *indent = indent.chars().skip(2).collect();
                println!("\n{}}};", indent);
                *indent = indent.chars().skip(2).collect();
                print!("\n{}}}", indent);
                for key in node.list_keys() {
                    print!(")");
                }
                println!(";");
                println!("\n{}default = {{}};", indent);
                *indent = indent.chars().skip(2).collect();
                println!("{}}};", indent);
            }

            SchemaNodeKind::Choice => {
                println!("\n{}{} = {{", indent, node.name());
                *indent += "  ";
                for child in node.children() {
                    print_nix_options(indent, child);
                }
                *indent = indent.chars().skip(2).collect();
                println!("{}}};", indent);
            }

            SchemaNodeKind::Case => {
                println!("\n{}{} = {{", indent, node.name());
                *indent += "  ";
                for child in node.children() {
                    print_nix_options(indent, child);
                }
                *indent = indent.chars().skip(2).collect();
                println!("{}}};", indent);
            }

            SchemaNodeKind::Leaf | SchemaNodeKind::LeafList => {
                println!("\n{}{} = lib.mkOption {{", indent, node.name());
                if let Some(description) = node.description() {
                    println!("{}  description = \"{}\";", indent, description);
                };
                let leaf_type = match node.base_type() {
                    Some(DataValueType::Enum) => "lib.types.str",
                    Some(DataValueType::Union) => "lib.types.str",
                    Some(DataValueType::String) => "lib.types.str",
                    Some(DataValueType::Int8) => "lib.types.ints.s8",
                    Some(DataValueType::Uint8) => "lib.types.ints.u8",
                    Some(DataValueType::Uint16) => "lib.types.ints.u16",
                    Some(DataValueType::Uint32) => "lib.types.ints.u32",
                    Some(DataValueType::Uint64) => "lib.types.ints.unsigned",
                    Some(DataValueType::Dec64) => "lib.types.number",
                    other => todo!("{:?}", other),
                };
                match node.kind() {
                    SchemaNodeKind::Leaf if !node.is_mandatory() => {
                        println!("{}  type = lib.types.nullOr {};", indent, leaf_type)
                    }
                    SchemaNodeKind::Leaf => println!("{}  type = {};", indent, leaf_type),
                    SchemaNodeKind::LeafList => {
                        println!("{}  type = lib.types.listOf {};", indent, leaf_type)
                    }
                    _ => unreachable!(),
                }
                match node.kind() {
                    SchemaNodeKind::Leaf if !node.is_mandatory() => {
                        println!("{}  default = null;", indent)
                    }
                    SchemaNodeKind::LeafList => println!("{}  default = [];", indent),
                    _ => {}
                }
                println!("{}}};", indent);
            }
            other => todo!("{:?}", other),
        }
    }
}

fn set_color(op: yang2::data::DataDiffOp) {
    match op {
        yang2::data::DataDiffOp::Create => {
            print!("\x1b[92m+ ");
        }
        yang2::data::DataDiffOp::Delete => {
            print!("\x1b[91m- ");
        }
        yang2::data::DataDiffOp::Replace => {
            print!("\x1b[93m~ ");
        }
    }
}

fn reset_color() {
    print!("\x1b[0m");
}

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();

    drop(args.next());

    let mode = match args.next().as_ref().map(|x| x.as_str()) {
        Some("yang2nix") => Mode::Convert(
            ConvertMode::Yang2Nix,
            std::fs::File::open(&args.next().expect("filename")).expect("realpath"),
        ),
        Some("nix2yang") => Mode::Convert(
            ConvertMode::Nix2Yang,
            std::fs::File::open(&args.next().expect("filename")).expect("realpath"),
        ),
        Some("nix_options") => Mode::NixOptions,
        Some("diff") => Mode::Diff(
            std::fs::File::open(&args.next().expect("filename")).expect("realpath"),
            std::fs::File::open(&args.next().expect("filename")).expect("realpath"),
        ),
        _ => panic!("mode: yang2nix nix2yang"),
    };

    std::env::set_current_dir(std::env::var("YANG_SCHEMAS_DIR").expect("env var YANG_SCHEMAS_DIR"))
        .expect("Failed to set YANG search directory");
    // Initialize context.
    let mut ctx = Context::new(ContextFlags::NO_YANGLIBRARY).expect("Failed to create context");

    ctx.load_module("rtbrick-config", None, &[])
        .expect("Failed to load module");

    //for module in ctx.modules(false) {
    //    eprintln!("loaded module {}@{:?}", module.name(), module.revision());
    //}

    let ctx = Arc::new(ctx);

    let module = ctx.get_module_latest("rtbrick-config").unwrap();

    let roots = module.data();

    let (mode, file) = match mode {
        Mode::Convert(mode, file) => (mode, file),
        Mode::NixOptions => {
            println!("{{ lib, ... }}: {{");
            let mut indent = "  ".to_string();
            for root in roots {
                print_nix_options(&mut indent, root);
            }
            println!("}}");
            std::process::exit(0);
        }
        Mode::Diff(file1, file2) => {
            use yang2::data::{
                Data, DataDiffFlags, DataFormat, DataParserFlags, DataPrinterFlags, DataTree,
                DataValidationFlags,
            };

            // Parse data trees from JSON strings.
            let dtree1 = DataTree::parse_file(
                &ctx,
                file1,
                DataFormat::JSON,
                DataParserFlags::NO_VALIDATION,
                DataValidationFlags::empty(),
            )
            .expect("Failed to parse data tree");

            let dtree2 = DataTree::parse_file(
                &ctx,
                file2,
                DataFormat::JSON,
                DataParserFlags::NO_VALIDATION,
                DataValidationFlags::empty(),
            )
            .expect("Failed to parse data tree");

            // Compare data trees.
            let diff = dtree1
                .diff(&dtree2, DataDiffFlags::empty())
                .expect("Failed to compare data trees");

            let dtree1_root = dtree1.reference();
            let dtree2_root = dtree2.reference();

            for (op, dnode) in diff.iter() {
                set_color(op);
                println!("{:?} @{}", op, dnode.path());
                let diffs_to_print = match op {
                    yang2::data::DataDiffOp::Replace => vec![
                        (
                            yang2::data::DataDiffOp::Delete,
                            dtree1_root
                                .as_ref()
                                .unwrap()
                                .find_path(&dnode.path())
                                .unwrap(),
                        ),
                        (
                            yang2::data::DataDiffOp::Create,
                            dtree2_root
                                .as_ref()
                                .unwrap()
                                .find_path(&dnode.path())
                                .unwrap(),
                        ),
                    ],
                    yang2::data::DataDiffOp::Delete => vec![(
                        op,
                        dtree1_root
                            .as_ref()
                            .unwrap()
                            .find_path(&dnode.path())
                            .unwrap(),
                    )],
                    yang2::data::DataDiffOp::Create => vec![(
                        op,
                        dtree2_root
                            .as_ref()
                            .unwrap()
                            .find_path(&dnode.path())
                            .unwrap(),
                    )],
                };

                for (op, dnode) in diffs_to_print {
                    let diff_str = dnode
                        .print_string(DataFormat::JSON, DataPrinterFlags::empty())
                        .expect("Failed to print data diff")
                        .unwrap();
                    for line in diff_str.lines() {
                        set_color(op);
                        println!("{}", line);
                    }
                }
                println!();
            }
            reset_color();
            std::process::exit(0);
        }
    };

    let mut data: serde_json::Value = serde_json::from_reader(BufReader::new(file))?;

    for node in roots
        .flat_map(|root| root.traverse().collect::<Vec<_>>().into_iter().rev())
        // only lists that have keys
        .filter(|node| node.kind() == SchemaNodeKind::List && !node.is_keyless_list())
    {
        let key_names = node
            .list_keys()
            .map(|ch| format!("{}", ch.name()))
            .collect::<Vec<_>>();
        let mut p = vec![&mut data];

        let mut ancestors = node
            .inclusive_ancestors()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .enumerate();
        let ancestors_len = ancestors.len();

        for (i, an) in &mut ancestors {
            let k = if i == 0 {
                format!("rtbrick-config:{}", an.name())
            } else {
                format!("{}", an.name())
            };

            p = p
                .into_iter()
                .flat_map(|x| x.get_mut(&k).into_iter())
                .collect();

            // last ancestor
            if i == (ancestors_len - 1) {
                // we didn't find anything
                if p.len() == 0 {
                    break;
                }

                // last node ; convert
                //println!("{:?} {:?}", node.path(SchemaPathFormat::DATA), key);
                for e in &mut p {
                    match mode {
                        ConvertMode::Yang2Nix => {
                            let as_array = if let serde_json::Value::Array(a) = e.take() {
                                a
                            } else {
                                panic!("Expected an array. Are you sure this is a YANG-style file?")
                            };

                            for mut el in as_array {
                                let mut p2 = &mut **e; // reference to the value where the element will be inserted
                                for key in &key_names {
                                    let k = el
                                        .as_object_mut()
                                        .expect("expected an object")
                                        .remove(key)
                                        .expect("expected key");
                                    let k = k
                                        .as_str()
                                        .map(|s| s.to_string())
                                        .or(k
                                            .as_number()
                                            .and_then(|n| serde_json::to_string(n).ok()))
                                        .expect("can not determine key");

                                    if !p2.is_object() {
                                        *p2 = serde_json::Value::Object(Default::default());
                                    };
                                    p2 = p2
                                        .as_object_mut()
                                        .unwrap()
                                        .entry(k)
                                        .or_insert(serde_json::Value::Null);
                                }
                                *p2 = el; // insert element
                            }
                        }
                        ConvertMode::Nix2Yang => {
                            let mut a = vec![];

                            let mut q: Vec<(Vec<String>, _)> = vec![(vec![], e.take())];

                            while let Some((depth, mut el)) = q.pop() {
                                if depth.len() == key_names.len() {
                                    for (key, key_name) in depth.into_iter().zip(node.list_keys()) {
                                        let key = match key_name.base_type() {
                                            Some(
                                                DataValueType::Int8
                                                | DataValueType::Int16
                                                | DataValueType::Int32
                                                | DataValueType::Int64
                                                | DataValueType::Uint8
                                                | DataValueType::Uint16
                                                | DataValueType::Uint32
                                                | DataValueType::Uint64
                                                | DataValueType::Dec64,
                                            ) => serde_json::from_str(&key).unwrap(),
                                            _ => serde_json::Value::from(key.to_string()),
                                        };
                                        el.as_object_mut()
                                            .expect("expected object")
                                            .insert(key_name.name().to_string(), key);
                                    }
                                    a.push(el);
                                } else {
                                    //println!("{:?}", depth);
                                    //println!("{:?}", el);
                                    let as_object = if let serde_json::Value::Object(o) = el.take()
                                    {
                                        o
                                    } else {
                                        panic!("Expected an object. Are you sure this is a Nix-style file?");
                                    };
                                    for (key, el2) in as_object {
                                        let mut depth = depth.clone();
                                        depth.push(key);
                                        q.push((depth, el2));
                                    }
                                }
                            }

                            **e = serde_json::Value::Array(a);
                        }
                    }

                    //println!("{:?}", e);
                }
                //println!("");
                break;
            }
            if an.kind() == SchemaNodeKind::List {
                for _ in an.list_keys() {
                    p = p
                        .into_iter()
                        .flat_map(|x| -> Box<dyn Iterator<Item = &mut serde_json::Value>> {
                            match x {
                                serde_json::Value::Array(a) => Box::new(a.into_iter()),
                                serde_json::Value::Object(o) => Box::new(o.values_mut()),
                                _ => panic!(),
                            }
                        })
                        .collect();
                }
            }

            if p.len() == 0 {
                break;
            }
        }
    }

    println!("{}", serde_json::to_string(&data).unwrap());

    Ok(())
}
