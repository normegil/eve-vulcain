# Eve Vulcain
Eve Vulcain will help you to compute and plan industrial activities in Eve Online. Based on preregistered facilities,  markets, and items, it help you compare profits and select the best item to manufacture.

<p align="center">
    <img src="https://github.com/normegil/eve-vulcain/assets/3015686/79845d12-16d7-4206-9a28-b9622506b655">
</p>

## Download
Currently, only binaries are available. You can find them in the [release section](https://github.com/normegil/eve-vulcain/releases).

Download the binary corresponding to your operating system, and put it somewhere in your $PATH.

## Setup
The first time usage required a call to

```
eve-vulcain init
```

It will help you download the [eve online SDE](https://developers.eveonline.com/resource) locally, perform the first time login and let you start to add facilities and items used in the application. 

## Usages
Right now, you can access theses commands:
* `state`: Display your current ISK amount, as well as your orders and running jobs. 
* `manufacture all`: Compute the manufacturing costs and profits of all registered items, using registered markets & facilities. Sort the results by profits per hour and display the average quantity sold for the last 30 days. 
* `manufacture item <ITEM NAME>`: Compute the manufacturing costs and profits of a specific items. Gives details on the calculation. 
* `invent item <ITEM NAME>`: Compute the invention costs of a specific items (and associated blueprint). Gives details on the calculation, the cost is normalized using the computed probability of success.
* `facility add/rm`: Manage registered facilities.
* `item add/rm`: Manage registered items.
