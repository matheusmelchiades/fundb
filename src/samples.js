const figlet = require('figlet')
const Table = require('cli-table')

exports.intro = () => {
    const template = figlet.textSync('Fun DB', {
        font: 'Slant Relief',
        horizontalLayout: 'default',
        verticalLayout: 'default'
    })

    console.log('\n')
    console.log(template)
    console.log('\n')
}

console.table = (model = {}, data = []) => {
    const fields = Object.keys(model)
    const table = new Table({
        head: fields,
    })

    data.map(item => {
        const row = []
        fields.map(field => {
            row.push(item[field] || '')
        })

        table.push(row)
    })

    console.log(table.toString())
};