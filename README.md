# PagerDuty CLI

A tool to interact with PagerDuty.

## Example - Who Is Oncall

List who is currently oncall.

**Note:** You can get a working `PD_TOKEN` at https://api-reference.pagerduty.com/ for PagerDuty's Demo API

```sh
> pagerduty-cli -a $PD_TOKEN who-is-oncall
 ├─ Escilation Policy - Escalation Policy sint laudantium voluptates
 │  └─ Oncalls
 │     ├─ Level 1 - Carolina Bernier
 │     ├─ Level 2 - Gage Pfeffer, Nathan Durgan
 │     └─ Level 3 - Earlene Reinger
 ├─ Escilation Policy - Escalation Policy quos aut consequatur
 │  └─ Oncalls
 │     ├─ Level 1 - Courtney Vandervort, Erwin Veum, Sydnie Doyle, Constantin Cassin
 │     ├─ Level 2 - Alice O'Connell, Victoria Bauch
 │     └─ Level 3 - Alessandro Crona, Kristin Schulist
 └─ Escilation Policy - Escalation Policy perspiciatis pariatur numquam
    └─ Oncalls
       ├─ Level 1 - Sydnie Doyle, Courtney Vandervort, Noemie Ankunding, Royal Murazik, Jerel Effertz
       ├─ Level 2 - Marilie Schuster, Kaylah Ruecker
       └─ Level 3 - Nickolas Kunze, Haylie Ankunding
```

## Example - Export

Generate a JSON file that can be used in Terraform.

**Note:** You can get a working `PD_TOKEN` at https://api-reference.pagerduty.com/ for PagerDuty's Demo API

```sh
> pagerduty-cli -a $PD_TOKEN export
{
  "escalation_policies": {
    "Default": "P9OFD2O",
    "Escalation Policy accusamus eveniet ea": "P2EQYW3",
    "Escalation Policy accusantium et eveniet": "PPGU7XT",
    "Escalation Policy adipisci itaque velit": "P7DBLPX",
    "Escalation Policy alias doloribus ut": "PYIGXD9",
    "Escalation Policy alias qui consequatur": "P61Y5FC"
  ]
}