#!/bin/bash

PKG=$(brew leaves)


for i in ${PKG}; do
    case $i in 
        "node" | "go")
        echo keep $i
        ;;
        *)
        echo delete $i
        brew uninstall $i
        ;;
    esac
done

#sudo rm -rf /Library/Frameworks/Python.framework/Versions/2.7
#sudo rm -rf "/Applications/Python 2.7"
#ls -l /usr/local/bin | grep '../Library/Frameworks/Python.framework/Versions/2.7'
#cd /usr/local/bin/
#ls -l /usr/local/bin | grep '../Library/Frameworks/Python.framework/Versions/2.7' | awk '{print $9}' | tr -d @ | xargs rm