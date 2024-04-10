import yaml
import dataclasses


class SafeDataclassDumper(yaml.SafeDumper):
    def represent_data(self, data):
        if dataclasses.is_dataclass(data):
            return self.represent_dict(dataclasses.asdict(data))
        return super().represent_data(data)
