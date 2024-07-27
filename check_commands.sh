#!/bin/bash

function check_command() {
    echo 
    echo
    echo $i
    ${1} --version
    which $i
    ls -l $(which ${1})
}

check_command python
check_command python3
check_command pip
check_command pip3