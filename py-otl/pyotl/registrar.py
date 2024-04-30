import hashlib
from typing import Dict, ClassVar, List
from pyotl.interfaces import Target


class OtlRegistrar:
    registered_targets: ClassVar[Dict[str, "Target"]] = []


def hash_object(obj):
    field_values = [str(value) for value in obj.__dict__.values()]
    concatenated_values = "".join(field_values).encode("utf-8")
    return hashlib.sha256(concatenated_values).hexdigest()


def register(target, registry):
    tc_hash = hash_object(target)
    if tc_hash in target:
        raise RuntimeError(
            f"Colliding hashes for {target.name} -- there are two indentical targets that have been instantiated"
        )
    registry[tc_hash] = target


def register_simulator(target):
    register(target, OtlRegistrar.registered_simulators)


def get_all_registered_tests() -> List["Target"]:
    return list(OtlRegistrar.registered_targets.values())
