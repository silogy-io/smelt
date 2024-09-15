#!/bin/bash
if [ "$1" == "-h" ]; then
  echo "Utility to create sealed images for testing"
  echo "Argument 1: Container name"
  echo "Argument 2: New image name"
  echo "Argument 3: Command to execute"
  exit 0
fi
docker run --platform linux/amd64 --name $1 sealed_example smelt execute $3 --prepare-workspace
# wait for the container to finish its task (optional)
# add any necessary delay or wait logic here
# commit the changes to a new image
docker commit $1 $2
# stop and remove the original container
docker stop $1
docker rm $1
