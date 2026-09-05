use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use crate::application::drive_folder::DriveFolderOwner;
use crate::application::drive_tree::{DriveChild, FOLDER_MIME_TYPE, SHORTCUT_MIME_TYPE};
use crate::domain::GooglePermissionId;

pub const SOURCE_TOKEN: &str = "source-bearer-token";
pub const TARGET_TOKEN: &str = "target-bearer-token";
pub const SOURCE_PERM: &str = "perm-source";
pub const TARGET_PERM: &str = "perm-target";

pub fn spawn_http_handler<F>(handler: F) -> (String, Arc<Mutex<Vec<String>>>)
where
    F: Fn(&str) -> (String, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").expect("test server binds");
    let address = listener.local_addr().expect("test server has an address");
    let captured = Arc::new(Mutex::new(Vec::new()));
    let captured_thread = Arc::clone(&captured);
    let handler = Arc::new(handler);
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();

    thread::spawn(move || {
        let _ = ready_tx.send(());
        loop {
            let Ok((mut stream, _)) = listener.accept() else {
                break;
            };
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
            let mut request = Vec::new();
            let mut chunk = [0_u8; 2048];
            loop {
                match stream.read(&mut chunk) {
                    Ok(0) => break,
                    Ok(read) => {
                        request.extend_from_slice(&chunk[..read]);
                        if http_request_complete(&request) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            let request = String::from_utf8_lossy(&request).into_owned();
            if let Ok(mut guard) = captured_thread.lock() {
                guard.push(request.clone());
            }
            let (status, body) = handler(&request);
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });

    ready_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("http mock thread starts");
    (format!("http://{address}"), captured)
}

fn http_request_complete(request: &[u8]) -> bool {
    let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let headers = String::from_utf8_lossy(&request[..header_end]);
    let Some(length) = headers.lines().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())
                .flatten()
        })
    }) else {
        return true;
    };
    request.len().saturating_sub(header_end + 4) >= length
}

pub fn request_method(request: &str) -> Option<&str> {
    request.lines().next()?.split_whitespace().next()
}

pub fn request_path(request: &str) -> Option<&str> {
    request.lines().next()?.split_whitespace().nth(1)
}

pub fn request_body(request: &str) -> &str {
    match request.split_once("\r\n\r\n") {
        Some((_, body)) => body,
        None => "",
    }
}

pub fn authorization_bearer(request: &str) -> Option<String> {
    request.lines().find_map(|line| {
        line.split_once(':').and_then(|(name, value)| {
            name.eq_ignore_ascii_case("authorization")
                .then(|| value.trim().to_string())
        })
    })
}

pub fn request_is_list(request: &str) -> bool {
    request.starts_with("GET /drive/v3/files?")
}

pub fn request_is_quota(request: &str) -> bool {
    request.contains("/drive/v3/about?fields=storageQuota")
}

pub fn query_param(request: &str, key: &str) -> Option<String> {
    let line = request.lines().next()?;
    let path = line.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            return Some(url_decode(value));
        }
    }
    None
}

pub fn folder_id_from_list_request(request: &str) -> Option<String> {
    let q = query_param(request, "q")?;
    let rest = q.strip_prefix('\'')?;
    let (id, _) = rest.split_once("' in parents")?;
    Some(id.replace("\\'", "'").replace("\\\\", "\\"))
}

fn url_decode(value: &str) -> String {
    let mut bytes = Vec::with_capacity(value.len());
    let chars: Vec<char> = value.chars().collect();
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '+' => {
                bytes.push(b' ');
                index += 1;
            }
            '%' if index + 2 < chars.len() => {
                let hex: String = chars[index + 1..index + 3].iter().collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    bytes.push(byte);
                    index += 3;
                } else {
                    bytes.extend(chars[index].to_string().as_bytes());
                    index += 1;
                }
            }
            other => {
                bytes.extend(other.encode_utf8(&mut [0; 4]).as_bytes());
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn child_json(id: &str, name: &str, mime: &str, owner: &str, extra: &str) -> String {
    format!(
        r#"{{"id":"{id}","name":"{name}","mimeType":"{mime}","trashed":false,"parents":["parent"],"owners":[{{"permissionId":"{owner}","emailAddress":"owner@gmail.com"}}]{extra}}}"#
    )
}

pub fn source_file(id: &str, name: &str, quota: i64) -> String {
    child_json(
        id,
        name,
        "text/plain",
        SOURCE_PERM,
        &format!(r#","quotaBytesUsed":"{quota}""#),
    )
}

pub fn source_folder(id: &str, name: &str) -> String {
    child_json(id, name, FOLDER_MIME_TYPE, SOURCE_PERM, "")
}

pub fn shortcut_json(id: &str, target_id: &str) -> String {
    child_json(
        id,
        "Shortcut",
        SHORTCUT_MIME_TYPE,
        SOURCE_PERM,
        &format!(r#","shortcutDetails":{{"targetId":"{target_id}"}}"#),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn drive_child(
    id: &str,
    name: &str,
    mime: &str,
    owner: &str,
    drive_id: Option<&str>,
    shortcut_target: Option<&str>,
    quota: Option<i64>,
    trashed: bool,
) -> DriveChild {
    DriveChild {
        id: id.into(),
        name: name.into(),
        mime_type: mime.into(),
        parents: vec!["parent".into()],
        owners: vec![DriveFolderOwner {
            permission_id: GooglePermissionId::new(owner),
            email_address: Some("owner@gmail.com".into()),
        }],
        drive_id: drive_id.map(ToOwned::to_owned),
        quota_bytes_used: quota,
        trashed,
        shortcut_target_id: shortcut_target.map(ToOwned::to_owned),
    }
}
