import toml
import argparse
import subprocess

pypath = "py-smelt/pyproject.toml"
cargopath = "Cargo.toml"


def bump_version(version, bump_type):
    major, minor, patch = map(int, version.split("."))
    if bump_type == "major":
        major += 1
        minor = 0
        patch = 0
    elif bump_type == "minor":
        minor += 1
        patch = 0
    elif bump_type == "patch":
        patch += 1
    else:
        raise ValueError('Invalid bump type. Choose from "major", "minor", "patch".')
    return f"{major}.{minor}.{patch}"


def bump_pyproject(bump_type: str) -> str:

    with open(pypath, "r") as file:
        data = toml.load(file)
    new_version = bump_version(data["project"]["version"], bump_type)

    data["project"]["version"] = bump_version(data["project"]["version"], bump_type)
    with open(pypath, "w") as file:
        toml.dump(data, file)
    return new_version


def bump_cargo(bump_type: str):

    with open(cargopath, "r") as file:
        data = toml.load(file)
    data["workspace"]["package"]["version"] = bump_version(
        data["workspace"]["package"]["version"], bump_type
    )
    with open(cargopath, "w") as file:
        toml.dump(data, file)


def main():
    parser = argparse.ArgumentParser(
        description="Bump the version in pyproject.toml and cargo.toml"
    )
    parser.add_argument(
        "bump_type",
        choices=["major", "minor", "patch"],
        help="The part of the version to bump.",
    )
    parser.add_argument(
        "--push_git",
        action="store_true",
        help="If set, commit the changes and push to git.",
    )

    args = parser.parse_args()
    bump_type = args.bump_type
    out_version = bump_pyproject(bump_type)
    bump_cargo(bump_type)
    print(f"bumped to version {out_version}")

    if args.push_git:
        branch = f"release-{out_version}"
        subprocess.run(["git", "checkout", "-b", branch])
        subprocess.run(["taplo", "fmt", pypath, cargopath])
        subprocess.run(["git", "add", pypath, cargopath])
        subprocess.run(["git", "commit", "-m", f"Bump version to {out_version}"])
        subprocess.run(["git", "push", "origin", branch])
        subprocess.run(["git", "tag", out_version])
        subprocess.run(["git", "push", "origin", out_version])


if __name__ == "__main__":
    main()
