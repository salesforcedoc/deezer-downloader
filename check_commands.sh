#!/bin/bash

function check_command() {
    echo 
    echo
    echo "${1}"
    ${1} --version
    which "${1}"
    ls -l $(which ${1})
}

check_command python
check_command python3
check_command pip
check_command pip3