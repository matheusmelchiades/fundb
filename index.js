require('./src/samples').intro();
const rl = require('./config').rl
const actions = require('./src/actions')
const {
    find, help, insert, new_table, list_tables,
    use
} = require('./src/types')

rl.prompt('\n');

rl.on('line', (line) => {

    try {

        const commands = line.split(' ')

        const command = commands.shift().trim()

        switch (command) {
            case new_table.label:
                actions.createTable(commands)
                break;

            case use.label:
                actions.setTable(commands)
                break;

            case list_tables.label:
                actions.listTables()
                break;

            case find.label:
                actions.find(commands)
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

    } catch (error) {
        console.error(error.message);
    }

})

rl.on('close', () => {
    console.log('\n\n FUNDB Is THE BEST MOTHER FUCK!\n\n');
    process.exit(0);
});