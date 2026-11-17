use serde_json::{json, Value};
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    io::stdin()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("request-limit".into());
    }
    let request: Value = serde_json::from_slice(&bytes)?;
    if request["protocol"] != "lekalo/runtime/v1"
        || !request["request_id"]
            .as_str()
            .is_some_and(|id| id.len() == 71 && id.starts_with("sha256:"))
    {
        return Err("request-envelope".into());
    }
    let result = dispatch(
        request["operation"].as_str().ok_or("operation")?,
        &request["input"],
    )?;
    print!(
        "{}",
        json!({"protocol":"lekalo/runtime/v1","request_id":request["request_id"],"result":result})
    );
    Ok(())
}

use lekalo_typescript_discovery as d;
const ID: &str = "lekalo-typescript-discovery";
const FILES: &[&str] = &[
    "package.json",
    "tsconfig.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "bun.lockb",
    "bun.lock",
    "lerna.json",
    "turbo.json",
    "nx.json",
];
const READS: &[&str] = &["package.json", "pnpm-workspace.yaml"];

fn path_dir(path: &str) -> &str {
    path.rsplit_once('/').map(|(d, _)| d).unwrap_or(".")
}
fn dispatch(op: &str, input: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    if op == "describe" {
        return Ok(
            json!({"id":ID,"version":"0.1.0","rules_api":[],"capabilities":["discovery/v1"],"operations":["discovery.describe","discovery.inspect"]}),
        );
    }
    if op == "discovery.describe" {
        return Ok(json!({"schema":"lekalo/discovery/v1","files":FILES,"reads":READS}));
    }
    if op != "discovery.inspect" || input["schema"] != "lekalo/discovery/v1" {
        return Err("operation-input".into());
    }
    let files = input["files"].as_array().ok_or("files")?;
    if files.len() > 10000 {
        return Err("files-limit".into());
    }
    let mut out = json!({"schema":"lekalo/discovery/v1","manifests":[],"managers":[],"hints":[],"workspaces":[],"tools":[],"gates":[],"modules":[],"names":[],"refused":[]});
    for f in files {
        let path = f["path"].as_str().ok_or("path")?;
        let name = path.rsplit('/').next().ok_or("path")?;
        let text = f["content"].as_str();
        let value = &f["value"];
        for (file, ecosystem) in d::MANIFESTS {
            if name == *file {
                out["manifests"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"path":path,"detail":ecosystem}));
            }
        }
        inspect(name, path, text, value, &mut out);
    }
    finish(&mut out);
    Ok(out)
}
fn push(out: &mut Value, key: &str, value: Value) {
    out[key].as_array_mut().unwrap().push(value);
}
fn module(
    out: &mut Value,
    path: &str,
    name: Option<&str>,
    standalone: bool,
    priority: u8,
    source: &str,
) {
    push(
        out,
        "modules",
        json!({"path":path_dir(path),"manifest":path,"name":name,"standalone":standalone,"priority":priority}),
    );
    if let Some(name) = name {
        push(
            out,
            "names",
            json!({"path":path,"name":name,"source":source,"priority":priority}),
        );
    }
}

fn inspect(name: &str, path: &str, _: Option<&str>, value: &Value, out: &mut Value) {
    for (file, manager) in d::LOCKFILES {
        if name == *file {
            push(out, "managers", json!({"path":path,"detail":manager}));
        }
    }
    if name == "tsconfig.json" {
        push(
            out,
            "hints",
            json!({"label":"typescript","kind":"language","path":path,"field":null,"confidence":"medium"}),
        );
    }
    if d::MONOREPO_TOOLS.contains(&path) {
        push(
            out,
            "tools",
            json!({"path":path,"detail":"monorepo-tooling"}),
        );
    }
    if name == "pnpm-workspace.yaml" {
        if let Some(rows) = value["packages"].as_array() {
            let members: Vec<_> = rows.iter().filter_map(Value::as_str).collect();
            if !members.is_empty() {
                push(
                    out,
                    "workspaces",
                    json!({"path":path_dir(path),"ecosystem":"node","members":members,"confidence":"high"}),
                );
            }
        }
    }
    if name != "package.json" {
        return;
    }
    if !value.is_object() {
        push(out, "refused", json!(path));
        return;
    }
    for (label, kind, field) in d::dependency_hints(value) {
        push(
            out,
            "hints",
            json!({"label":label,"kind":kind,"path":path,"field":field,"confidence":"high"}),
        );
    }
    if let Some(members) = d::node_workspace_members(value) {
        push(
            out,
            "workspaces",
            json!({"path":path_dir(path),"ecosystem":"node","members":members,"confidence":"high"}),
        );
    }
    module(
        out,
        path,
        value["name"].as_str(),
        true,
        10,
        "package.json#name",
    );
    for script in d::node_scripts(value) {
        let gate = match script.as_str() {
            "build" | "compile" => "build",
            "test" => "test",
            "lint" => "lint",
            "check" | "ci" => "check",
            _ => continue,
        };
        push(
            out,
            "gates",
            json!({"gate":gate,"command":format!("npm run {script}"),"source":"workflow","evidence":format!("{path}#scripts.{script}"),"confidence":"high"}),
        );
    }
}
fn finish(out: &mut Value) {
    if out["manifests"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["path"] == "package.json")
    {
        out["gates"].as_array_mut().unwrap().retain(|r| {
            r["evidence"].as_str().unwrap().split('#').next().unwrap() <= "package.json"
        });
    }

    let mut managers = out["managers"].as_array().unwrap().clone();
    managers.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let manager = managers
        .iter()
        .filter_map(|r| r["detail"].as_str())
        .find(|m| d::MANAGERS.contains(m))
        .unwrap_or("npm")
        .to_owned();
    for gate in out["gates"].as_array_mut().unwrap() {
        if gate["evidence"]
            .as_str()
            .is_some_and(|s| s.starts_with("package.json#"))
        {
            gate["command"] = json!(gate["command"]
                .as_str()
                .unwrap()
                .replacen("npm", &manager, 1));
        }
    }
    let mut workspaces = out["workspaces"].as_array().unwrap().clone();
    workspaces.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    workspaces.dedup_by(|a, b| a["path"] == b["path"]);
    out["workspaces"] = json!(workspaces);
}
