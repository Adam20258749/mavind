# Mavind default .bashrc — minimal.
case $- in *i*) ;; *) return;; esac

HISTCONTROL=ignoreboth
HISTSIZE=1000
HISTFILESIZE=2000
shopt -s checkwinsize histappend

PS1='\[\e[1;35m\]\u@mavind\[\e[0m\]:\[\e[1;34m\]\w\[\e[0m\]\$ '

alias ls='ls --color=auto'
alias ll='ls -lh'
alias la='ls -lha'
alias grep='grep --color=auto'
alias mon='mavind-system-monitor'
alias win='mavind-wine'

# pipewire/wayland niceties for CLI-launched GUI apps
export MOZ_ENABLE_WAYLAND=1
export QT_QPA_PLATFORM=wayland
export GDK_BACKEND=wayland
