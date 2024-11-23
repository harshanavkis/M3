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

fn parse_single_line(line: &str) -> (String, BTreeMap<String, String>) {
    let mut result: BTreeMap<String, String> = BTreeMap::new();

    // Split the line into function name and predicates
    if let Some((func_name, predicates)) = line.split_once(":-") {
        let func_name = func_name.trim();
        let predicates = predicates.trim();

        // Parse individual predicates joined by '&'
        for predicate in predicates.split('&') {
            let predicate = predicate.trim();

            // Split predicate into key and value
            if let Some((key, value)) = predicate.split_once('(') {
                let value = value.trim_end_matches(')').trim();
                result.insert(key.to_string(), value.to_string());
            }
        }

        return (func_name.to_string(), result);
    }

    ("".to_string(), result) // Return empty values if parsing fails
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
                let (parsed_node_edge, parsed_policy) = parse_single_line(p);
                if policy_json.get(&parsed_node_edge).is_none() {
                    policy_json.insert(parsed_node_edge, vec![parsed_policy]);
                }
                else {
                    let policy_vec = policy_json.get_mut(&parsed_node_edge).unwrap();
                    policy_vec.push(parsed_policy);
                }
            }
        }
    }
    let attribute_json = node_map;

    (policy_json, attribute_json)
}

fn get_create_node_policy(policy_set: &Vec<BTreeMap<String, String>>) -> Option<(bool, String)> {
    for v in policy_set {
        if v.get("HWIsExclusive").is_some() {
            if let Some(node_name) = v.get("HWVersionIs") {
                return Some((true, node_name.to_string().replace("\"", "")));
            }
            else {
                Some((true, "CPU".to_string().replace("\"", "")));
            }
        }
        else {
            if let Some(node_name) = v.get("HWVersionIs") {
                return Some((false, node_name.to_string().replace("\"", "")));
            }
            return Some((false, "CPU".to_string().replace("\"", "")));
        }
    }

    None
}

fn get_edges(policy_set: &Vec<BTreeMap<String, String>>) -> Vec<(String, String)> {
    let mut res: Vec<(String, String)> = Vec::new();
    for i in policy_set {
        res.push((
            i.get("Src").unwrap().to_string(),
            i.get("Sink").unwrap().to_string(),
        ));
    }

    res
}

fn is_node_sw_service(node: &str, policy_set: &Vec<BTreeMap<String, String>>) -> bool {
    for v in policy_set {
        if v.get("SWVersionIs").is_some() {
            if v.get("SWVersionIs").unwrap() == node {
                return true;
            }
        }
    }

    false
}

pub fn compile_policy(user_policy: &str) -> bool {
    let (policy_json, attribute_json) = parse_policy(&user_policy);

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
            is_node_sw_service(i, policy_json.get("dfg_node").unwrap()),
        );
    }

    let edges = get_edges(policy_json.get("dfg_edge").unwrap());

    let pipeserv = wv_assert_ok!(Pipes::new("pipes"));

    for e in edges {
        if is_node_sw_service(&e.0, policy_json.get("dfg_node").unwrap())
            || is_node_sw_service(&e.1, policy_json.get("dfg_node").unwrap())
        {
            println!("Skip creating pipes");
            continue;
        }
        let pipe_mem = wv_assert_ok!(MemGate::new(0x10000, kif::Perm::RW));
        let pipe = wv_assert_ok!(IndirectPipe::new(&pipeserv, &pipe_mem, 0x10000));

        pipes.insert(e, pipe);
    }

    return true;
}
