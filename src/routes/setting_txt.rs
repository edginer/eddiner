use worker::*;

use crate::utils::response_shift_jis_text_plain;

pub fn route_setting_txt() -> Result<Response> {
    let builder = String::from(
        "liveedge@liveedge
    BBS_TITLE=エッヂ
    BBS_TITLE_ORIG=エッヂ
    BBS_NONAME_NAME=エッヂの名無し
    BBS_TITLE_PICTURE=//www2.5ch.net/5ch.gif
    BBS_TITLE_COLOR=#000000
    BBS_TITLE_LINK=//www.5ch.net/info.html
    BBS_BG_COLOR=#FFFFFF
    BBS_BG_PICTURE=//www2.5ch.net/ba.gif
    BBS_MAKETHREAD_COLOR=#CCFFCC
    BBS_MENU_COLOR=#CCFFCC
    BBS_THREAD_COLOR=#EFEFEF
    BBS_TEXT_COLOR=#000000
    BBS_NAME_COLOR=green
    BBS_LINK_COLOR=#0000FF
    BBS_ALINK_COLOR=#FF0000
    BBS_VLINK_COLOR=#660099
    BBS_THREAD_NUMBER=10
    BBS_CONTENTS_NUMBER=10
    BBS_LINE_NUMBER=16
    BBS_MAX_MENU_THREAD=10
    BBS_SUBJECT_COLOR=#FF0000
    BBS_UNICODE=pass
    BBS_NAMECOOKIE_CHECK=checked
    BBS_MAILCOOKIE_CHECK=checked
    BBS_SUBJECT_COUNT=96
    BBS_NAME_COUNT=64
    BBS_MAIL_COUNT=64
    BBS_MESSAGE_COUNT=4096
    BBS_THREAD_TATESUGI=8
    BBS_PROXY_CHECK=
    BBS_OVERSEA_PROXY=
    BBS_RAWIP_CHECK=
    BBS_SLIP=verbose
    BBS_DISP_IP=
    BBS_FORCE_ID=checked
    BBS_BE_ID=
    BBS_BE_TYPE2=
    BBS_NO_ID=
    BBS_JP_CHECK=
    BBS_YMD_WEEKS=
    EMOTICONS=checked
    BBS_NOSUSU=checked
    BBS_USE_VIPQ2=16
                ",
    );

    response_shift_jis_text_plain(builder)
}
