#!/bin/bash

function check_command() {
    ${1} --version
    ls -l $(which ${1})
}


check_command python
check_command python3
check_command pip
check_command pip3