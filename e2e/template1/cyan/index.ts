import { GlobType, StartTemplateWithLambda } from '@atomicloud/cyan-sdk';
import { IInquirer, IDeterminism } from '@atomicloud/cyan-sdk';
StartTemplateWithLambda(async (i: IInquirer, d: IDeterminism) => {
  const color = await i.select('What is your favorite color?', ['red', 'blue', 'green'], 'ernest/template1/color');
  const name = await i.text('What is your name?', 'ernest/template1/name');
  const food = await i.select('What is your favorite food?', ['pizza', 'burger', 'salad'], 'ernest/template1/food');
  const country = await i.select(
    'What is your favorite country?',
    ['USA', 'Canada', 'Mexico'],
    'ernest/template1/country',
  );
  const animal = await i.select('What is your favorite animal?', ['dog', 'cat', 'bird'], 'ernest/template1/animal');
  const sport = await i.select(
    'What is your favorite sport?',
    ['soccer', 'basketball', 'tennis'],
    'ernest/template1/sport',
  );
  const season = await i.select(
    'What is your favorite season?',
    ['spring', 'summer', 'fall', 'winter'],
    'ernest/template1/season',
  );
  const music = await i.select(
    'What is your favorite music genre?',
    ['rock', 'pop', 'jazz', 'classical'],
    'ernest/template1/music',
  );
  const hobby = await i.select(
    'What is your favorite hobby?',
    ['reading', 'gaming', 'cooking', 'hiking'],
    'ernest/template1/hobby',
  );

  return {
    processors: [
      {
        name: 'ernest/processor1',
        files: [
          {
            glob: '**/*',
            exclude: [],
            type: GlobType.Template,
            root: 'template',
          },
        ],
        config: {
          vars: {
            color,
            name,
            food,
            country,
            animal,
            sport,
            season,
            music,
            hobby,
          },
        },
      },
    ],
    plugins: [
      {
        name: 'ernest/plugin1',
        config: {},
      },
    ],
  };
});
