rm -rf packages
gh release download 0.0.0 -D packages -R silogy-io/otl
tar -czvf packages.tar.gz packages
rsync packages.tar.gz prod:~/
ssh prod
