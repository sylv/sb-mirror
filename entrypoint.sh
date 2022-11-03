#!/bin/ash

# start nginx
nginx -g 'daemon off;' &
  
# start the mirror
/usr/local/bin/sb-mirror &
  
# wait for either process to exit
wait -n
exit $?
