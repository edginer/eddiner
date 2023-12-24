use std::collections::HashMap;

pub(crate) mod auth;
pub(crate) mod auth_code;
pub(crate) mod bbs_cgi;
pub(crate) mod dat_routing;
pub(crate) mod head_txt;
pub(crate) mod setting_txt;
pub(crate) mod subject_txt;
pub(crate) mod webui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Route<'a> {
    Index,
    Auth,
    AuthCode,
    BbsCgi,
    Dat {
        board_key: &'a str,
        board_id: usize,
        thread_id: &'a str,
    },
    KakoDat {
        board_key: &'a str,
        board_id: usize,
        thread_id: &'a str,
    },
    SettingTxt {
        board_key: &'a str,
        board_id: usize,
    },
    SubjectTxt {
        board_key: &'a str,
        board_id: usize,
    },
    HeadTxt {
        board_key: &'a str,
        board_id: usize,
    },
    BoardIndex {
        board_key: &'a str,
        board_id: usize,
    },
    ThreadWebUI {
        board_key: &'a str,
        board_id: usize,
        thread_id: &'a str,
    },
    NotFound,
}

pub fn analyze_route<'a>(path: &'a str, board_keys: &'a HashMap<String, usize>) -> Route<'a> {
    match path {
        "/" | "/index.html" => Route::Index,
        "/auth/" | "/auth" => Route::Auth,
        "/auth-code/" | "/auth-code" => Route::AuthCode,
        "/test/bbs.cgi" => Route::BbsCgi,
        path => {
            if path.len() < 4 {
                return Route::NotFound;
            }
            let ext = &path[path.len() - 4..];
            match ext {
                ".dat" => {
                    // /:board_key/dat/:thread_id.dat OR /:board_key/kako/:thread_id_top_4/:thread_id_top_5/:thread_id.dat
                    let path = path.split(&['/', '.'][..]).collect::<Vec<_>>();

                    match path.len() {
                        5 => {
                            if path[2] != "dat" || path[3].len() != 10 {
                                return Route::NotFound;
                            }
                            let board_key = path[1];
                            if let Some(board_id) = board_keys.get(board_key) {
                                Route::Dat {
                                    board_key,
                                    board_id: *board_id,
                                    thread_id: path[3],
                                }
                            } else {
                                Route::NotFound
                            }
                        }
                        7 => {
                            let (top_4, top_5, thread_id) = (path[3], path[4], path[5]);
                            if path[2] != "kako" {
                                return Route::NotFound;
                            };
                            if top_4 != &thread_id[..4]
                                || top_5 != &thread_id[..5]
                                || thread_id.len() != 10
                            {
                                return Route::NotFound;
                            };
                            let board_key = path[1];
                            if let Some(board_id) = board_keys.get(board_key) {
                                Route::KakoDat {
                                    board_key,
                                    board_id: *board_id,
                                    thread_id,
                                }
                            } else {
                                Route::NotFound
                            }
                        }
                        _ => Route::NotFound,
                    }
                }
                ".TXT" | ".txt" => {
                    let split = path.split('/').collect::<Vec<_>>();
                    if split.len() != 3 {
                        return Route::NotFound;
                    }
                    let board_key = split[1];
                    let board_id = board_keys.get(board_key).copied();
                    match (split[2], board_id) {
                        ("SETTING.TXT", Some(board_id)) => Route::SettingTxt {
                            board_key,
                            board_id,
                        },
                        ("subject.txt", Some(board_id)) => Route::SubjectTxt {
                            board_key,
                            board_id,
                        },
                        ("head.txt", Some(board_id)) => Route::HeadTxt {
                            board_key,
                            board_id,
                        },
                        _ => Route::NotFound,
                    }
                }
                _ => {
                    // /:board_key/:thread_id/? OR /test/read.cgi/:board_key/:thread_id/? OR /:board_key/?
                    let mut split = path.split('/').collect::<Vec<_>>();
                    if split.last() == Some(&"") {
                        split.pop();
                    }
                    match split.len() {
                        2 => {
                            let board_key = split[1];
                            if let Some(board_id) = board_keys.get(board_key) {
                                Route::BoardIndex {
                                    board_key,
                                    board_id: *board_id,
                                }
                            } else {
                                Route::NotFound
                            }
                        }
                        3 => {
                            let (board_key, thread_id) = (split[1], split[2]);
                            if thread_id.len() != 10 {
                                return Route::NotFound;
                            }
                            if let Some(board_id) = board_keys.get(board_key) {
                                Route::ThreadWebUI {
                                    board_key,
                                    board_id: *board_id,
                                    thread_id,
                                }
                            } else {
                                Route::NotFound
                            }
                        }
                        5 => {
                            let (board_key, thread_id) = (split[3], split[4]);
                            if split[1] != "test" || split[2] != "read.cgi" || thread_id.len() != 10
                            {
                                return Route::NotFound;
                            }
                            if let Some(board_id) = board_keys.get(board_key) {
                                Route::ThreadWebUI {
                                    board_key,
                                    board_id: *board_id,
                                    thread_id,
                                }
                            } else {
                                Route::NotFound
                            }
                        }
                        _ => Route::NotFound,
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_board_keys() -> HashMap<String, usize> {
        let mut map = HashMap::new();
        map.insert("liveedge".to_string(), 1);
        map
    }

    #[test]
    fn test_const_path() {
        let paths = ["/", "/auth", "/auth-code", "/test/bbs.cgi"];
        let expecteds = [Route::Index, Route::Auth, Route::AuthCode, Route::BbsCgi];

        for (path, expected) in paths.iter().zip(expecteds.iter()) {
            assert_eq!(analyze_route(path, &HashMap::new()), *expected);
        }
    }

    #[test]
    fn test_dat() {
        let paths = [
            "/liveedge/dat/1666666666.dat",
            "/liveedge/dat/fewdgerfef.dat", // invalid, but should be parsed as a path in analyze_route
            "/liveedge/kako/1666/16666/1666666666.dat",
            "/liveedge/dat/1666666666222.dat",
        ];
        let expecteds = [
            Route::Dat {
                board_key: "liveedge",
                thread_id: "1666666666",
                board_id: 1,
            },
            Route::Dat {
                board_key: "liveedge",
                thread_id: "fewdgerfef",
                board_id: 1,
            },
            Route::KakoDat {
                board_key: "liveedge",
                thread_id: "1666666666",
                board_id: 1,
            },
            Route::NotFound,
        ];

        for (path, expected) in paths.iter().zip(expecteds.iter()) {
            assert_eq!(analyze_route(path, &generate_board_keys()), *expected);
        }
    }

    #[test]
    fn test_txt() {
        let paths = ["/liveedge/SETTING.TXT", "/liveedge/subject.txt"];
        let expecteds = [
            Route::SettingTxt {
                board_key: "liveedge",
                board_id: 1,
            },
            Route::SubjectTxt {
                board_key: "liveedge",
                board_id: 1,
            },
        ];

        for (path, expected) in paths.iter().zip(expecteds.iter()) {
            assert_eq!(analyze_route(path, &generate_board_keys()), *expected);
        }
    }

    #[test]
    fn test_thread_web_ui() {
        let paths = [
            "/liveedge/1666666667/",
            "/test/read.cgi/liveedge/1666666668/",
            "/liveedge/1666666669",
            "/test/read.cgi/liveedge/1666666666",
        ];
        let expecteds = [
            Route::ThreadWebUI {
                board_key: "liveedge",
                thread_id: "1666666667",
                board_id: 1,
            },
            Route::ThreadWebUI {
                board_key: "liveedge",
                thread_id: "1666666668",
                board_id: 1,
            },
            Route::ThreadWebUI {
                board_key: "liveedge",
                thread_id: "1666666669",
                board_id: 1,
            },
            Route::ThreadWebUI {
                board_key: "liveedge",
                thread_id: "1666666666",
                board_id: 1,
            },
        ];

        for (path, expected) in paths.iter().zip(expecteds.iter()) {
            assert_eq!(analyze_route(path, &generate_board_keys()), *expected);
        }
    }
}
