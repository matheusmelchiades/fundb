require('./src/samples').intro();
const logger = require('./src/logger')
const rl = require('./config').rl
const actions = require('./src/actions')
const {
    find, help, insert, new_table, list_tables,
    use, checkpoint, commit, findById, update
} = require('./src/types')

rl.prompt('\n');

logger.watchSystem();

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

            case findById.label:
                actions.findById(commands)
                break;

            case insert.label:
                actions.insert(commands)
                break;

            case checkpoint.label:
                actions.checkpoint();
                break;

            case commit.label:
                actions.commit();
                break;

            case help.label:
                actions.help(commands, rl)
                break;

            case update.label:
                actions.update(commands)
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
    actions.exit()
    console.log('\n\n FUNDB Is THE BEST MOTHER FUCK!\n\n');
    process.exit(0);
});