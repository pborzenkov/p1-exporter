# `p1-exporter` - Prometheus exporter for DSMR reader with serial over TCP

`p1-exporter` is a Prometheus exporter for DMSR (Dutch Smart Meter Requirements)
reader with serial over TCP.

The following metrics are currently exported:

```
# HELP p1_power_consumed_watts Power consumed.
# TYPE p1_power_consumed_watts gauge
# HELP p1_power_produced_watts Power produced.
# TYPE p1_power_produced_watts gauge
# HELP p1_power_consumed_watts_total Total consumed power.
# TYPE p1_power_consumed_watts_total counter
# HELP p1_power_produced_watts_total Total produced power.
# TYPE p1_power_produced_watts_total counter
# HELP p1_active_tariff Currently active tariff.
# TYPE p1_active_tariff gauge
# HELP p1_gas_consumed_cubic_meters_total Total consumed natural gas.
# TYPE p1_gas_consumed_cubic_meters_total counter
```

## License

Licensed under [MIT license](LICENSE)
