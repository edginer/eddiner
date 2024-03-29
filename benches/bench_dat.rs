use criterion::{criterion_group, criterion_main, Bencher, Criterion};

use eddiner::response::{Ch5ResponsesFormatter, Res};

fn format_responses_string(responses: &[Res], thread_title: &str, default_name: &str) -> String {
    let mut builder = String::new();

    for (idx, r) in responses.iter().enumerate() {
        builder.push_str(&format!(
            "{}<>{}<>{} ID:{}<> {} <>{}",
            r.name
                .clone()
                .map(|x| if x.is_empty() {
                    default_name.to_string()
                } else {
                    x
                })
                .unwrap_or_else(|| default_name.to_string())
                .replace('\n', ""),
            r.mail
                .clone()
                .unwrap_or_else(|| "".to_string())
                .replace('\n', ""),
            r.date,
            r.author_id.clone().unwrap_or_else(|| "".to_string()),
            r.body.replace('\n', "<br>"),
            if idx == 0 {
                thread_title.replace('\n', "")
            } else {
                "".to_string()
            }
        ));
        builder.push('\n');
    }

    builder
}

fn generate_responses() -> Vec<Res> {
    let mut responses = Vec::new();
    for i in 0..1000 {
        let name = match i % 3 {
            0 => Some("コテハン".to_owned()),
            1 => Some("".to_owned()),
            _ => None,
        };
        let min = (i / 60) % 60;
        let sec = i % 60;
        let body = (0..i).fold(String::new(), |b, _| b + "レス\n");
        let res = Res {
            name,
            mail: None,
            date: format!("2099/9/09(金) 0:{}:{}.00", min, sec),
            author_id: Some(format!("abC/DEf{}", sec)),
            body,
            thread_id: "1666666666".to_owned(),
            ip_addr: "1.1.1.1".to_owned(),
            authed_token: None,
            timestamp: 0,
            is_abone: 0,
        };
        responses.push(res);
    }
    responses
}

fn generate_dat_string(b: &mut Bencher<'_>) {
    let responses = generate_responses();
    b.iter(|| {
        let _dat = responses.format_responses("スレタイ", "デフォ名無し");
    });
}
fn generate_dat_string_shift_jis(b: &mut Bencher<'_>) {
    let responses = generate_responses();
    b.iter(|| {
        let dat = format_responses_string(&responses, "スレタイ", "デフォ名無し");
        let _ = encoding_rs::SHIFT_JIS.encode(&dat).0;
    });
}

fn criterion_benchmark(c: &mut Criterion) {
    let mut group = c.benchmark_group("dat-format");
    group.sample_size(10000);
    group.bench_function("dat_string", generate_dat_string);
    group.bench_function("dat_string_encoding", generate_dat_string_shift_jis);
    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
