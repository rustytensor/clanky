# Clanky's shell integration for bash, started as: bash --init-file <this file> -i
# It reads ~/.bashrc as bash itself would, then adds OSC 133 prompt marks (A, B before the
# command, C as it starts, D;status after it) and OSC 7 folder reports. With the window's
# command input in editor mode the prompt prints only the marks; in classic mode the
# user's own PS1 is printed between them. Nothing here forks at a prompt.
if [[ -r ~/.bashrc ]]; then . ~/.bashrc; fi
if [[ -z ${__clanky_loaded-} && -n ${CLANKY_SHELL_DIR-} ]] && (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4) )); then
__clanky_loaded=1
__clanky_osc7() {
    local LC_ALL=C p=$PWD out= c i
    for (( i = 0; i < ${#p}; i++ )); do
        c=${p:i:1}
        case $c in
            [A-Za-z0-9/._~-]) out+=$c ;;
            *) printf -v c '%%%02X' "'$c"; out+=$c ;;
        esac
    done
    printf '\e]7;file://%s%s\a' "${HOSTNAME-}" "$out"
}
__clanky_precmd() {
    local s=$?
    printf '\e]133;D;%s\a' "$s"
    __clanky_osc7
    return $s
}
__clanky_prompt() {
    local s=$?
    # The user (or a prompt theme) changed PS1 since the last prompt: keep theirs.
    [[ $PS1 != "${__clanky_ps1-}" ]] && __clanky_user_ps1=$PS1
    if [[ -e $CLANKY_SHELL_DIR/mode-$CLANKY_TAB ]]; then
        __clanky_ps1='\[\e]133;A\a\e]133;B\a\]'
    else
        __clanky_ps1='\[\e]133;A\a\]'"${__clanky_user_ps1-}"'\[\e]133;B\a\]'
    fi
    PS1=$__clanky_ps1
    [[ ${PROMPT_COMMAND[0]} == __clanky_precmd ]] || PROMPT_COMMAND=(__clanky_precmd "${PROMPT_COMMAND[@]}")
    [[ $PS0 == *$'\e]133;C\a'* ]] || PS0=$'\e]133;C\a'"${PS0-}"
    return $s
}
if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a"* ]]; then
    PROMPT_COMMAND=(__clanky_precmd "${PROMPT_COMMAND[@]}" __clanky_prompt)
elif (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
    PROMPT_COMMAND=(__clanky_precmd ${PROMPT_COMMAND:+"$PROMPT_COMMAND"} __clanky_prompt)
else
    PROMPT_COMMAND="__clanky_precmd${PROMPT_COMMAND:+; $PROMPT_COMMAND}; __clanky_prompt"
fi
PS0=$'\e]133;C\a'"${PS0-}"
bind 'set enable-bracketed-paste on' 2>/dev/null
fi
