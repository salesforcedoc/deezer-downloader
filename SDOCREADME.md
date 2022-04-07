### Instructions
- install HTTP-TRACKER chrome web-extension
- run start.sh
- open http://localhost:5000 in chrome
- filter ".net/media" in HTTP-TRACKER
- click to song
- extract "media" URL
- wget "media" URL

python3 

python3 app2.py -u https://cdnt-proxy-7.dzcdn.net/media/1/b83e8e97ba05a2aaa2838e6ba3febcc376bee896ce1c0af8a996b6ba559ff606ba16161ff4ba999c97469b960dbb37a1547833ce79f9cdb6c6943d5d4d9edb9bb8f490046c1d8b3c158010413ae7ebd1?hdnea=exp=1646601984~acl=/media/1/b83e8e97ba05a2aaa2838e6ba3febcc376bee896ce1c0af8a996b6ba559ff606ba16161ff4ba999c97469b960dbb37a1547833ce79f9cdb6c6943d5d4d9edb9bb8f490046c1d8b3c158010413ae7ebd1*~data=user_id=1923762066~hmac=6e607fe54b23a37f2ee77af8df72182645f6d31a470cf10b5ea7f72a23521755#99c5e164-a7a1-4607-b55a-82e66a3aeea8