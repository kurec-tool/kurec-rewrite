#!/bin/bash

docker stop $(docker ps | grep nats | cut -f 1 -d\  )
docker stop $(docker ps | grep toxi | cut -f 1 -d\  )

docker network rm $(docker network ls | grep nats_test_net | cut -f 1 -d\  )
