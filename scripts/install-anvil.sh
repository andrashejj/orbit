# script to install anvil and make sure it's available in $PATH

# install anvil and say always yes
curl -L https://foundry.paradigm.xyz | bash
source ~/.bashrc

# run foundryup to make sure foundry is installed
foundryup

# add anvil to $PATH
export PATH="$PATH:$HOME/.foundry/bin"

