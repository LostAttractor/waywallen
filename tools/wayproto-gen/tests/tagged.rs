use std::{fs, process::Command};

#[test]
fn tagged_rust_and_c_share_vectors() {
    let xml = r#"<protocol name="t" version="9">
      <enum name="mode" open="true"><entry name="known" value="1"/></enum>
      <struct name="config" encoding="tagged">
        <field name="title" type="string" tag="3"/>
        <field name="mode" type="mode" tag="1"/>
      </struct>
      <request name="probe" opcode="14" encoding="tagged">
        <arg name="config" type="config" tag="1" optional="true"/>
        <arg name="names" type="array" element="string" tag="2" optional="true"/>
      </request>
      <event name="empty" opcode="1" encoding="tagged"/>
    </protocol>"#;
    let dir = std::env::temp_dir().join(format!("wayproto-tagged-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("ww_proto.h"),
        wayproto_gen::emit_c_header_from_xml(xml).unwrap(),
    )
    .unwrap();
    fs::write(
        dir.join("ww_proto.c"),
        wayproto_gen::emit_c_source_from_xml(xml).unwrap(),
    )
    .unwrap();
    let rust = wayproto_gen::emit_rust_from_xml(xml).unwrap();
    fs::write(dir.join("wire.rs"), rust).unwrap();
    fs::write(dir.join("check.rs"), r#"
#[path="wire.rs"] mod wire;
fn main() {
    let bytes = std::fs::read(std::env::args().nth(1).unwrap()).unwrap();
    let result = wire::Request::decode(14, &bytes);
    assert_eq!(result.is_ok(), std::env::args().nth(2).unwrap() == "ok");
    if let Ok(value) = result { let mut out = Vec::new(); value.encode(&mut out); std::fs::write("rust.bin", out).unwrap(); }
}
"#).unwrap();
    fs::write(
        dir.join("check.c"),
        r#"
#include "ww_proto.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
int main(int argc, char **argv) {
    assert(argc == 3);
    FILE *f = fopen(argv[1], "rb"); assert(f);
    unsigned char bytes[70000]; size_t len = fread(bytes, 1, sizeof(bytes), f); fclose(f);
    ww_req_probe_t value;
    int rc = ww_req_probe_decode(bytes, len, &value);
    assert((rc == WW_OK) == (strcmp(argv[2], "ok") == 0));
    if (!rc) {
        ww_buf_t out; ww_buf_init(&out); assert(ww_req_probe_encode(&value, &out) == WW_OK);
        f = fopen("c.bin", "wb"); assert(f); fwrite(out.data, 1, out.len, f); fclose(f);
        ww_req_probe_free(&value); ww_buf_free(&out);
    }
}
"#,
    )
    .unwrap();
    for (compiler, args) in [
        (
            "rustc",
            vec![
                "--edition=2021",
                "-Awarnings",
                "check.rs",
                "-o",
                "check-rust",
            ],
        ),
        (
            "cc",
            vec![
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wno-unused-function",
                "ww_proto.c",
                "check.c",
                "-o",
                "check-c",
            ],
        ),
    ] {
        let out = Command::new(compiler)
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    fn words(values: &[u32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }
    // Nested config has unknown enum 99 and the UTF-8 title "a".
    let valid = words(&[1, 56, 1, 11, 44, 1, 36, 1, 12, 4, 99, 3, 8, 8, 2, 97]);
    let mut unknown = valid.clone();
    unknown[4..8].copy_from_slice(&72u32.to_le_bytes());
    unknown.extend(words(&[99, 999, 4, 123]));
    let mut duplicate = valid.clone();
    duplicate[4..8].copy_from_slice(&72u32.to_le_bytes());
    duplicate.extend(words(&[1, 11, 4, 0]));
    let mut bad_type = valid.clone();
    bad_type[12..16].copy_from_slice(&8u32.to_le_bytes());
    let mut bad_padding = valid.clone();
    *bad_padding.last_mut().unwrap() = 1;
    let mut bad_utf8 = valid.clone();
    let n = bad_utf8.len();
    bad_utf8[n - 4] = 0xff;
    let mut future = valid.clone();
    future[0..4].copy_from_slice(&2u32.to_le_bytes());
    let mut reordered = valid[..28].to_vec();
    reordered.extend_from_slice(&valid[44..]);
    reordered.extend_from_slice(&valid[28..44]);
    let mut bad_nested_length = valid.clone();
    bad_nested_length[24..28].copy_from_slice(&100u32.to_le_bytes());
    for (index, (bytes, ok)) in [
        (valid.clone(), true),
        (unknown, true),
        (duplicate, false),
        (bad_type, false),
        (bad_padding, false),
        (bad_utf8, false),
        (future, true),
        (words(&[1, 0]), true),
        (valid[..valid.len() - 1].to_vec(), false),
        (reordered, true),
        (bad_nested_length, false),
        (words(&[1, 16, 2, 264, 4, 1025]), false),
        (words(&[1, 16, 2, 264, 4, 0]), true),
    ]
    .into_iter()
    .enumerate()
    {
        let input = dir.join(format!("{index}.bin"));
        fs::write(&input, bytes).unwrap();
        for checker in ["check-rust", "check-c"] {
            let output = Command::new(dir.join(checker))
                .arg(&input)
                .arg(if ok { "ok" } else { "bad" })
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{checker} vector {index}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if ok {
            assert_eq!(
                fs::read(dir.join("rust.bin")).unwrap(),
                fs::read(dir.join("c.bin")).unwrap()
            );
            if index == 0 || index == 1 || index == 9 {
                assert_eq!(fs::read(dir.join("rust.bin")).unwrap(), valid);
            }
        }
    }
}

#[test]
fn tagged_schema_rejects_ambiguous_fields_and_fd_ownership() {
    for body in [
        r#"<request name="x" opcode="1" encoding="tagged"><arg name="a" type="u32"/></request>"#,
        r#"<request name="x" opcode="1" encoding="tagged"><arg name="a" type="u32" tag="0"/></request>"#,
        r#"<request name="x" opcode="1" encoding="tagged"><arg name="a" type="u32" tag="1"/><arg name="b" type="u32" tag="1"/></request>"#,
        r#"<request name="x" opcode="1"><arg name="a" type="u32" optional="true"/></request>"#,
        r#"<request name="x" opcode="1" encoding="tagged"><fds count="1"/></request>"#,
        r#"<struct name="old"><field name="x" type="u32"/></struct><request name="x" opcode="1" encoding="tagged"><arg name="a" type="old" tag="1"/></request>"#,
    ] {
        let xml = format!(r#"<protocol name="t" version="9">{body}</protocol>"#);
        assert!(wayproto_gen::emit_rust_from_xml(&xml).is_err(), "{xml}");
    }
}
