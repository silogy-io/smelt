from dataclasses import dataclass
from pysmelt.interfaces import Target, TargetRef
import functools
from typing import Callable, List, Dict, Optional, Tuple, Union
from copy import deepcopy

from pysmelt.interfaces.command import Command
from pysmelt.smelt_muncher import target_to_command

RerunCallback = Callable[[Target, int], Tuple[Target, bool]]


@dataclass
class DerivedTarget:
    origin: Target
    """
    The original target that
    """
    derived_target: Target
    """
    The target that was derived from the `origin` target, with none of its dependencies updated

    Because the dependencies aren't updated 
    """
    requires_rerun: bool
    """
    true if the origin target requires rerun 
    """

    @classmethod
    def from_cb(cls, origin: Target, cb: Tuple[Target, bool]):
        return cls(origin=origin, derived_target=cb[0], requires_rerun=cb[1])

    @functools.cached_property
    def derived_is_different(self) -> bool:
        """ """
        return "\n".join(self.derived_target.gen_script()) != "\n".join(
            self.origin.gen_script()
        )

    def get_new_command(
        self, all_derived: Dict[TargetRef, "DerivedTarget"]
    ) -> Optional[Command]:
        """ """

        dependenents = [
            all_derived[dep].get_new_command(all_derived)
            for dep in self.derived_target.get_dependencies()
        ]
        return self.lower_derived_target(dependenents)

    def lower_derived_target(
        self, dependenents: List[Union[None, Command]]
    ) -> Optional[Command]:
        deps_changed = any(dependenents)
        if deps_changed:
            """ """
            rv = target_to_command(self.derived_target)

            def choose(new: Optional[Command], old: str) -> str:
                return new.name if new else old

            new_deps = [
                choose(new_dep, old_dep)
                for new_dep, old_dep in zip(dependenents, rv.dependencies)
            ]
            rv.dependencies = new_deps
            return rv
        elif self.derived_is_different or self.requires_rerun:
            return target_to_command(self.derived_target)
        else:
            return None
