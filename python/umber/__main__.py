"""Install or uninstall the bundled skill with ``python -m umber skill``."""

import sys

from umber._skill_cli import main

if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
