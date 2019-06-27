const config = require('../config');
const types = require('./types')
const sgdb = require('./sgdb');
const fundb = new sgdb(config.path);

const noTable = () => console.log('\nTable not selected!\n');

exports.exit = () => {
    fundb.exit()
}

exports.createTable = ([table]) => {

    if (!table)
        return console.log('Input invalid!')

    fundb.createTable(table)

}

exports.findById = ([index]) => {

    if (!fundb.getCurrentTable())
        return noTable()

    if (!index) {
        console.log('\n')
        console.log('Please inform a id')
        console.log('\n')
    }

    const data = fundb.findById(index);

    if (data.length) {
        console.log('\n')
        console.table(data[0], data)
        console.log('\n')
    } else {
        console.log('\n')
        console.log('Data not found')
        console.log('\n')
    }
}

exports.find = ([]) => {
    if (!fundb.getCurrentTable())
        return noTable()

    const data = fundb.find()

    if (!data.length) {
        console.log('\n')
        console.log('TABLE EMPTY')
        console.log('\n')
    } else {
        console.log('\n')
        console.table(data[0], data)
        console.log('\n')
        return
    }
}

exports.insert = ([description]) => {

    if (!fundb.getCurrentTable())
        return noTable()

    fundb.insert({ description });
}

exports.update = ([id, description]) => {

    if (!fundb.getCurrentTable())
        return noTable()

    fundb.update({ id: parseInt(id), description });
}

exports.setTable = ([name]) => {

    const table = fundb.setCurrentTable(name)

    if (!table)
        return console.log('\nTable not exists\n')

    config.rl.setPrompt(`${config.samplePrompt} 📂\  ${table} 👉 `)
}

exports.listTables = () => {

    const tables = fundb.listTables();

    if (!tables.length)
        return console.log('\nNo exists tables!\n')

    console.log('\n')
    tables.map(table => console.log(`  📂\ >> ${table}`))
    console.log('\n')

}

exports.checkpoint = () => {

    if (!fundb.getCurrentTable())
        return noTable()

    fundb.checkpoint()
}

exports.commit = () => {

    if (!fundb.getCurrentTable())
        return noTable()

    fundb.commit()
}

exports.help = () => {

    console.log('\n')

    for (const action in types) {

        console.log(types[action].help, '\n')

    }

}
