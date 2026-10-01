set -eu
umask 077
test ! -e /home/ubuntu/mir2-native-upload-0f74c17b589e41f697be17b448bd403e
mkdir -m 700 -- /home/ubuntu/mir2-native-upload-0f74c17b589e41f697be17b448bd403e
stat -c "%a %U:%G %n" -- /home/ubuntu/mir2-native-upload-0f74c17b589e41f697be17b448bd403e
