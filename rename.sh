find . ! -path './.git*' -type f -exec grep -l 'smelt\|Smelt\|SMELT' {} \; | while read file
do
    sed -i '' 's/smelt/smelt/g;s/Smelt/Smelt/g;s/SMELT/SMELT/g' "$file"
done

# Rename files and directories
find . ! -path './.git*' -depth -name '*smelt*' -execdir bash -c 'mv -- "$1" "${1//smelt/smelt}"' bash {} \;
find . ! -path './.git*' -depth -name '*Smelt*' -execdir bash -c 'mv -- "$1" "${1//Smelt/Smelt}"' bash {} \;
find . ! -path './.git*' -depth -name '*SMELT*' -execdir bash -c 'mv -- "$1" "${1//SMELT/SMELT}"' bash {} \;

