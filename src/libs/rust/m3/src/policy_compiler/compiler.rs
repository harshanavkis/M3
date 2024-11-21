use base::boxed::Box;
use base::col::BTreeMap;
use base::col::BTreeSet;
use base::col::String;
use base::col::ToString;
use base::errors::Error;
use base::vec;
use base::vec::Vec;

use crate::com::MemGate;
use crate::session::Pipes;
use crate::syscalls;
use crate::vfs::IndirectPipe;
use base::kif;

macro_rules! wv_assert_ok {
    ($res:expr) => {{
        let res = $res;
        match res {
            Ok(r) => r,
            Err(e) => {
                println!(
                    "! {}:{}  expected Ok for {}, got {:?} FAILED",
                    file!(),
                    line!(),
                    stringify!($res),
                    e
                );
                panic!("wv_assert_ok failed")
            },
        }
    }};
}

fn parse_policies(input: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut result: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    // Extract the function name (assumed to be the first word before ':')
    let function_name = input.split(':').next().map(|s| s.trim()).unwrap_or("");

    // Find the stage number
    let stage_number = input
        .split(" & ")
        .find(|x| x.contains("Stage"))
        .and_then(|stage_capture| {
            let start = stage_capture.find('(').unwrap_or(0);
            stage_capture[start + 1..]
                .find(')')
                .map(|end| stage_capture[start + 1..start + 1 + end].to_string())
        })
        .unwrap_or_default();

    // Create a new BTreeMap to store policy details
    let mut json_object: BTreeMap<String, String> = BTreeMap::new();

    // Add the function name
    json_object.insert("function".to_string(), function_name.to_string());

    // Find and extract function calls and their arguments
    let mut current_func_name = String::new();
    let mut current_args = String::new();
    let mut in_func_call = false;
    let mut paren_count = 0;

    for ch in input.chars() {
        match ch {
            '(' if !in_func_call => {
                in_func_call = true;
                paren_count = 1;
                current_func_name = current_func_name.trim().to_string();
            },
            ')' if in_func_call => {
                paren_count -= 1;
                if paren_count == 0 {
                    // Store the function call and its arguments
                    if !current_func_name.is_empty() {
                        json_object
                            .insert(current_func_name.clone(), current_args.trim().to_string());

                        // Reset for next function call
                        current_func_name.clear();
                        current_args.clear();
                        in_func_call = false;
                    }
                }
            },
            ch if in_func_call => {
                if ch == '(' {
                    paren_count += 1;
                }
                current_args.push(ch);
            },
            ch if ch.is_alphabetic() && !in_func_call => {
                current_func_name.push(ch);
            },
            _ => {},
        }
    }

    // Add the constructed object to the result if stage number is found
    if !stage_number.is_empty() {
        result.insert(stage_number, json_object);
    }

    result
}

fn combine_json_objects(
    new_obj: BTreeMap<String, BTreeMap<String, String>>,
    old_obj: &mut BTreeMap<String, Vec<BTreeMap<String, String>>>,
) {
    // Make sure the old_obj is a mutable object
    let combined = old_obj; // Get a mutable reference to the underlying map

    for (key, value) in new_obj {
        combined
            .entry(key.clone())
            .and_modify(|e| {
                // if let Some(arr) = e {
                e.push(value.clone());
                // }
            })
            .or_insert_with(|| vec![value.clone()].into());
    }
}

fn parse_dict(attr_list: &str) -> Result<(String, BTreeMap<String, String>), Error> {
    // TODO: Add parsing capabilities for json lists
    let mut map: BTreeMap<String, String> = BTreeMap::new();

    let (key, json_part) = attr_list
        .split_once('=')
        .map(|(key, json)| (key.trim(), json.trim()))
        .unwrap(); // Trim spaces around key and value
                   // .ok_or("Invalid format: missing '='")?;

    let values = &json_part[1..json_part.len() - 1];

    for kv in values.split(",") {
        let arr: Vec<&str> = kv.trim().split(":").collect();
        map.insert(
            arr[0].to_string().replace("\"", ""),
            arr[1].to_string().replace("\"", ""),
        );
    }

    Ok((key.to_string(), map))
}

fn parse_policy(
    policy_str: &str,
) -> (
    BTreeMap<String, Vec<BTreeMap<String, String>>>,
    BTreeMap<String, BTreeMap<String, String>>,
) {
    let mut node_map: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    let mut policy_json: BTreeMap<String, Vec<BTreeMap<String, String>>> = BTreeMap::new();

    let policies = policy_str.split("\n");

    for p in policies {
        if p.len() != 0 && !p.contains("#") {
            if p.contains("=") {
                let (k, m) = parse_dict(&p).unwrap();
                node_map.insert(k, m);
            }
            if p.contains(":-") {
                let parsed_policy = parse_policies(p);
                combine_json_objects(parsed_policy, &mut policy_json);
            }
        }
    }
    let attribute_json = node_map;

    (policy_json, attribute_json)
}

fn get_create_node_policy(
    policy_set: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
    stage: &str,
) -> Option<(bool, String)> {
    if let Some(policies) = policy_set.get(stage) {
        for v in policies {
            if v.get("function").unwrap() == "create_node" {
                if v.get("HWIsExclusive").is_some() {
                    if let Some(node_name) = v.get("HWVersionIs") {
                        return Some((true, node_name.to_string().replace("\"", "")));
                    }
                    else {
                        return Some((true, "CPU".to_string().replace("\"", "")));
                    }
                }
            }
            else {
                return Some((false, "CPU".to_string().replace("\"", "")));
            }
        }
    }

    None
}

fn get_sink(
    parent: &str,
    policy_set: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
) -> Vec<(String, String)> {
    let mut sinks: Vec<(String, String)> = Vec::new();

    // println!("{}", parent);

    if let Some(node_policies) = policy_set.get(parent) {
        for p in node_policies {
            if p.get("function").unwrap() == "send" {
                sinks.push((
                    parent.to_string(),
                    p.get("Sink").unwrap().to_string().replace("\"", ""),
                ));
            }
        }
    }
    sinks
}

fn is_node_sw_service(
    node: &str,
    policy_set: &BTreeMap<String, Vec<BTreeMap<String, String>>>,
) -> bool {
    if let Some(node_policies) = policy_set.get(node) {
        for p in node_policies {
            if p.get("function").unwrap() == "create_node" {
                if p.get("SWVersionIs").is_some() {
                    return true;
                }
                if p.get("HWVersionIs").is_some() {
                    return false;
                }
            }
        }
    }
    false
}

pub fn compile_policy(user_policy: &str) -> bool {
    let (policy_json, attribute_json) = parse_policy(&user_policy);
    // println!("{:?}", policy_json);

    // println!("{:?}", attribute_json);

    // for k in attribute_json.keys() {
    //     println!("{}", k);
    // }

    // if let Some((exclusive, node_name)) = get_create_node_policy(&policy_json, "GPU") {
    //     println!(
    //         "{:?}, {:?}",
    //         exclusive,
    //         attribute_json.get(&node_name).unwrap().get("ca").unwrap()
    //     );
    // }

    let mut edges: BTreeSet<(String, String)> = BTreeSet::new();

    let mut pipes: BTreeMap<(String, String), IndirectPipe> = BTreeMap::new();

    let ca = "ca".to_string();
    let hash = "hash".to_string();

    for i in attribute_json.keys() {
        if i == "MAIN" {
            continue;
        }

        let i_ca = attribute_json.get(i).unwrap().get("ca").unwrap();
        let i_hash = attribute_json.get(i).unwrap().get("hash").unwrap();
        let i_name = attribute_json.get(i).unwrap().get("name").unwrap();

        syscalls::attest(
            i_ca.to_string(),
            i_hash.to_string(),
            i_name.to_string(),
            is_node_sw_service(i, &policy_json),
        );
    }

    for i in policy_json.keys() {
        let edge = get_sink(i, &policy_json);
        for e in edge {
            edges.insert(e);
        }
    }

    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));

    for e in edges {
        // println!("{:?}", e);
        let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
        let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

        pipes.insert(e, pipe);
    }

    // println!("{:?}", edges);

    // println!("{}", is_node_sw_service("GPU", &policy_json));

    return true;
}
