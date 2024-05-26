#!/bin/bash

SESSION_NAME="timer_1"

tmux has-session -t $SESSION_NAME 2>/dev/null

if [ $? != 0 ]; then
  tmux new-session -d -s $SESSION_NAME -n 'Timer'

  tmux split-window -h -t $SESSION_NAME:0
  tmux split-window -v -t $SESSION_NAME:0.0
  tmux split-window -v -t $SESSION_NAME:0.2
  tmux split-window -h -t $SESSION_NAME:0.2

  tmux send-keys -t $SESSION_NAME:0.0 'rclock -D' C-m
  tmux send-keys -t $SESSION_NAME:0.1 "rclock -d '0:0:10' -x 'paplay files/notification.wav'" C-m
  tmux send-keys -t $SESSION_NAME:0.3 "rclock -c 3 -t Asia/Manila" C-m
  tmux select-pane -t $SESSION_NAME:0.2
  tmux send-keys -t $SESSION_NAME:0.2 'rclock' C-m
fi

# Подключение к сессии
tmux attach-session -t $SESSION_NAME
