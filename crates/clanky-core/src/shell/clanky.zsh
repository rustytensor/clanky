# Clanky's prompt marks for zsh: OSC 133 A, B around the prompt, C before a command runs,
# D;status after it, and OSC 7 folder reports. In editor mode the prompt is the marks only.
if [[ -z ${__clanky_loaded-} ]]; then
typeset -g __clanky_loaded=1 __clanky_ps1= __clanky_user_ps1= __clanky_user_rps1=
__clanky_osc7() {
    emulate -L zsh
    setopt extendedglob
    local LC_ALL=C
    print -rn -- $'\e]7;file://'"${HOST-}${PWD//(#m)[^A-Za-z0-9\/._~-]/%${(l:2::0:)$(( [##16] #MATCH ))}}"$'\a'
}
__clanky_precmd() {
    local s=$?
    print -rn -- $'\e]133;D;'"$s"$'\a'
    __clanky_osc7
    return $s
}
__clanky_preexec() { print -rn -- $'\e]133;C\a' }
__clanky_prompt() {
    local s=$?
    if [[ $PS1 != "$__clanky_ps1" ]]; then __clanky_user_ps1=$PS1; __clanky_user_rps1=${RPS1-}; fi
    if [[ -e $CLANKY_SHELL_DIR/mode-$CLANKY_TAB ]]; then
        __clanky_ps1=$'%{\e]133;A\a\e]133;B\a%}'
        RPS1=
    else
        __clanky_ps1=$'%{\e]133;A\a%}'"$__clanky_user_ps1"$'%{\e]133;B\a%}'
        RPS1=$__clanky_user_rps1
    fi
    PS1=$__clanky_ps1
    (( ${precmd_functions[(I)__clanky_precmd]} == 1 )) || precmd_functions=(__clanky_precmd ${precmd_functions:#__clanky_precmd})
    return $s
}
typeset -ga precmd_functions preexec_functions
precmd_functions=(__clanky_precmd $precmd_functions __clanky_prompt)
preexec_functions+=(__clanky_preexec)
fi
