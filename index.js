const readline = require('readline');
const prompt = '🚀 FunDB  👉 '
const actions = require('./src/actions')
const load = require('./src/load');

const rl = readline.createInterface({
    input: process.stdin,
    output: process.stdout,
    prompt
});

const { find, help, insert, new_table, show_tables } = require('./src/types')

load();

rl.prompt('\n');

rl.on('line', (line) => {

    try {

        const commands = line.split(' ')

        const command = commands.shift().trim()

        switch (command) {
            case new_table.label:
                actions.createTable(commands)
                break;

            case show_tables.label:
                actions.listTables()
                break;

            case find.label:
                actions.findByIndex(commands)
                break;
                
            case insert.label:
                actions.insert(commands)
                break;

            case help.label:
                actions.help(commands, rl)
                break;

            default:
                console.log('\nCommand invalid!\n')
                break;
        }

        rl.prompt('\n');

    } catch(error) {
        console.error(error.message);
    }

})

rl.on('close', () => {
    console.log('\n\n FUNDB Is THE BEST MOTHER FUCK!\n\n');
    process.exit(0);
});